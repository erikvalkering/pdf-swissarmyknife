use std::collections::{BTreeSet, HashSet, BTreeMap};
use std::fs::File;
use std::path::PathBuf;
use std::io::{BufReader, BufRead, Write};

use clap::{Parser, Subcommand, Args};
use itertools::Itertools;
use lopdf::Document;
use lopdf::Object;
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

fn main() {
    let args = AppArgs::parse();
    match &args.command {
        Command::Index(args) => index(args),
    }
}
