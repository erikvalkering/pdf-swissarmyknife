use std::collections::{BTreeSet, HashSet, BTreeMap};
use std::fs::File;
use std::path::PathBuf;
use std::io::{BufReader, BufRead, Write};

use clap::{Parser, Subcommand, Args};
use itertools::Itertools;
use lopdf::{Document, Object, ObjectId, Bookmark};
use colored::Colorize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct AppArgs {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate an index for a given pdf file
    Index(IndexArgs),

    /// Join several pdf files into a single document
    Join(JoinArgs),
}

#[derive(Args, Debug)]
struct IndexArgs {
    /// Path to PDF to generate an index for
    #[arg(short, long, default_value = "input.pdf")]
    pdf: PathBuf,

    /// Path file to write index to
    #[arg(short, long, default_value = "index.txt")]
    output: PathBuf,

    /// Path to file containing the words to be considered
    #[arg(short, long, default_value = "words.txt", num_args(0..=1))]
    words: Option<PathBuf>,

    /// Disable trimming of parentheses and allow non-alphabetic characters
    #[arg(short, long, default_value_t = false)]
    no_filtering: bool,

    /// Insert newlines after encountering end of text block
    #[arg(short, long, default_value_t = false)]
    insert_newlines: bool,

    /// Full text search mode (within a page)
    #[arg(short, long, default_value_t = false)]
    full_text: bool,

    /// Specify which pages to extract
    #[arg(long, num_args = 1..)]
    pages: Option<Vec<u32>>,

    /// Dump full extracted page text (for debugging)
    #[arg(short, long, default_value_t = false)]
    dump: bool,
}

#[derive(Args, Debug)]
struct JoinArgs {
    /// Path to PDFs to be joined
    pdfs: Vec<PathBuf>,

    /// Path to the resulting joined PDF
    #[arg(short, long, default_value = "output.pdf")]
    output: PathBuf,
}

fn load_words(path: &PathBuf) -> HashSet<String> {
    println!("{}", "Loading words file...".green());
    let words_file = File::open(path).expect("Cannot open words.txt containing the words to look for in the PDF");
    let words: HashSet<_> = BufReader::new(words_file)
        .lines()
        .map(|x| x.expect("Unable to parse a line from the words.txt"))
        .collect();

    words
}

fn extract_text(self_: &Document, page_numbers: &[u32], insert_newlines: bool) -> lopdf::Result<String> {
        fn collect_text(text: &mut String, encoding: Option<&str>, operands: &[Object]) {
            for operand in operands.iter() {
                match *operand {
                    Object::String(ref bytes, _) => {
                        let decoded_text = Document::decode_text(encoding, bytes);
                        text.push_str(&decoded_text);
                    }
                    Object::Array(ref arr) => {
                        collect_text(text, encoding, arr);
                        text.push(' ');
                    }
                    Object::Integer(i) => {
                        if i < -100 {
                            text.push(' ');
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut text = String::new();
        let pages = self_.get_pages();
        for page_number in page_numbers {
            let page_id = *pages.get(page_number).ok_or(lopdf::Error::PageNumberNotFound(*page_number))?;
            let fonts = self_.get_page_fonts(page_id);
            let encodings = fonts
                .into_iter()
                .map(|(name, font)| (name, font.get_font_encoding()))
                .collect::<BTreeMap<Vec<u8>, &str>>();
            let content_data = self_.get_page_content(page_id)?;
            let content = lopdf::content::Content::decode(&content_data)?;
            let mut current_encoding = None;
            for operation in &content.operations {
                match operation.operator.as_ref() {
                    "Tf" => {
                        let current_font = operation
                            .operands
                            .get(0)
                            .ok_or_else(|| lopdf::Error::Syntax("missing font operand".to_string()))?
                            .as_name()?;
                        current_encoding = encodings.get(current_font).cloned();
                    }
                    "Tj" | "TJ" => {
                        collect_text(&mut text, current_encoding, &operation.operands);
                    }
                    "ET" => {
                        if !text.ends_with('\n') && insert_newlines {
                            text.push('\n');
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(text)
    }

fn split_words(no_filtering: bool, text: &str, words: &Option<HashSet<String>>) -> Vec<(String, String)> {
    let mut result = vec![];
    for word in text.split_whitespace() {
        let word = if !no_filtering {
            let word = word.trim_matches(|c| !char::is_alphabetic(c));
            if word.is_empty() { continue; }
            if !word.chars().all(char::is_alphabetic) { continue; }

            word
        }
        else { word };

        let key = word.to_lowercase();
        if match words {
            Some(words) => !words.contains(&key),
            None => false,
        } {
            continue;
        }

        result.push((key, word.to_owned()));
    }

    result
}

fn full_text(text: &str, words: &HashSet<String>) -> Vec<(String, String)> {
    let mut result = vec![];
    let text = text.to_lowercase();
    for word in words {
        let key = word.to_lowercase();
        let key = key.trim();
        if key.is_empty() { continue; }

        if text.contains(key) {
            result.push((key.to_owned(), word.to_owned()));
        }
    }

    result
}

fn extract_index(args: &IndexArgs) -> BTreeMap<std::string::String, Vec<(std::string::String, u32)>> {
    println!("{}", "Reading pdf...".green());
    let doc = Document::load(&args.pdf).expect("Unable to open PDF");

    let words = args.words.as_ref().map(|x| load_words(&x));
    let words = if !args.full_text { words.map(|words| words.into_iter().map(|word| word.to_lowercase()).collect()) }
                                               else { words };

    println!("{}", "Extracting words from pages...".green());
    let mut index = BTreeMap::new();
    for (page_number, _) in doc.get_pages() {
        if args.pages.as_ref().map_or(false, |pages| !pages.contains(&page_number)) { continue; };

        let text = extract_text(&doc, &[page_number], args.insert_newlines).unwrap_or_else(|_| panic!("Unable to extract text from page {} from PDF", page_number));

        if args.dump {
            println!("page {}\n{}\n", page_number, text.blue());
        }

        let matches = if args.full_text {
            full_text(&text, &words.as_ref().expect("Cannot perform full text search without a word list"))
        }
        else {
            split_words(args.no_filtering, &text, &words)
        };

        for (key, word) in matches {
            let entry = &mut *index.entry(key).or_insert(vec![]);
            entry.push((word, page_number));
        }
    }

    index
}

fn index(args: &IndexArgs) {
    let index = extract_index(&args);

    println!("{}", "Writing to output file...".green());
    let mut output = File::create(&args.output).expect("Unable to create output file");
    for pages in index.values() {
        let unique_words: HashSet<_> = pages.iter().map(|(word, _)| word).collect();
        let page_numbers: BTreeSet<_> = pages.iter().map(|(_, page)| page).collect();

        writeln!(
            output,
            "{}: {}",
            unique_words.iter().join(", "),
            page_numbers.iter().join(", "),
        ).expect("Unable to write index entry to output");
    }
}

fn join(args: &JoinArgs) {
    // Generate a stack of Documents to merge
    let documents: Vec<_> = args.pdfs.iter().map(|pdf| Document::load(pdf).expect("Unable to open PDF")).collect();

    // Define a starting max_id (will be used as start index for object_ids)
    let mut max_id = 1;
    let mut pagenum = 1;
    // Collect all Documents Objects grouped by a map
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for mut doc in documents {
        let mut first = false;
        doc.renumber_objects_with(max_id);

        max_id = doc.max_id + 1;

        documents_pages.extend(
            doc
                    .get_pages()
                    .into_iter()
                    .map(|(_, object_id)| {
                        if !first {
                            let bookmark = Bookmark::new(String::from(format!("Page_{}", pagenum)), [0.0, 0.0, 1.0], 0, object_id);
                            document.add_bookmark(bookmark, None);
                            first = true;
                            pagenum += 1;
                        }

                        (
                            object_id,
                            doc.get_object(object_id).unwrap().to_owned(),
                        )
                    })
                    .collect::<BTreeMap<ObjectId, Object>>(),
        );
        documents_objects.extend(doc.objects);
    }

    // Catalog and Pages are mandatory
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    // Process all objects except "Page" type
    for (object_id, object) in documents_objects.iter() {
        // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects
        // All other objects should be collected and inserted into the main Document
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                // Collect a first "Catalog" object and use it for the future "Pages"
                catalog_object = Some((
                    if let Some((id, _)) = catalog_object {
                        id
                    } else {
                        *object_id
                    },
                    object.clone(),
                ));
            }
            "Pages" => {
                // Collect and update a first "Pages" object and use it for the future "Catalog"
                // We have also to merge all dictionaries of the old and the new "Pages" object
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref object)) = pages_object {
                        if let Ok(old_dictionary) = object.as_dict() {
                            dictionary.extend(old_dictionary);
                        }
                    }

                    pages_object = Some((
                        if let Some((id, _)) = pages_object {
                            id
                        } else {
                            *object_id
                        },
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            "Page" => {}     // Ignored, processed later and separately
            "Outlines" => {} // Ignored, not supported yet
            "Outline" => {}  // Ignored, not supported yet
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    // If no "Pages" object found abort
    if pages_object.is_none() {
        println!("Pages root not found.");

        return;
    }

    // Iterate over all "Page" objects and collect into the parent "Pages" created before
    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_object.as_ref().unwrap().0);

            document
                    .objects
                    .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // If no "Catalog" found abort
    if catalog_object.is_none() {
        println!("Catalog root not found.");

        return;
    }

    let catalog_object = catalog_object.unwrap();
    let pages_object = pages_object.unwrap();

    // Build a new "Pages" with updated fields
    if let Ok(dictionary) = pages_object.1.as_dict() {
        let mut dictionary = dictionary.clone();

        // Set new pages count
        dictionary.set("Count", documents_pages.len() as u32);

        // Set new "Kids" list (collected from documents pages) for "Pages"
        dictionary.set(
            "Kids",
            documents_pages
                    .into_iter()
                    .map(|(object_id, _)| Object::Reference(object_id))
                    .collect::<Vec<_>>(),
        );

        document
                .objects
                .insert(pages_object.0, Object::Dictionary(dictionary));
    }

    // Build a new "Catalog" with updated fields
    if let Ok(dictionary) = catalog_object.1.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_object.0);
        dictionary.remove(b"Outlines"); // Outlines not supported in merged PDFs

        document
                .objects
                .insert(catalog_object.0, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_object.0);

    // Update the max internal ID as wasn't updated before due to direct objects insertion
    document.max_id = document.objects.len() as u32;

    // Reorder all new Document objects
    document.renumber_objects();

     //Set any Bookmarks to the First child if they are not set to a page
    document.adjust_zero_pages();

    //Set all bookmarks to the PDF Object tree then set the Outlines to the Bookmark content map.
    if let Some(n) = document.build_outline() {
        if let Ok(x) = document.get_object_mut(catalog_object.0) {
            if let Object::Dictionary(ref mut dict) = x {
                dict.set("Outlines", Object::Reference(n));
            }
        }
    }

    document.compress();

    // Save the merged PDF
    document.save(&args.output).unwrap();
}

fn main() {
    let args = AppArgs::parse();
    match &args.command {
        Command::Index(args) => index(args),
        Command::Join(args) => join(args),
    }
}
