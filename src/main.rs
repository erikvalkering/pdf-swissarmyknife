use std::collections::{BTreeSet, HashSet, BTreeMap};
use std::fs::File;
use std::path::PathBuf;
use std::io::{BufReader, BufRead, Write};

use clap::Parser;
use itertools::Itertools;
use lopdf::Document;
use colored::Colorize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
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
}

fn load_words(path: &PathBuf) -> HashSet<String> {
    println!("{}", "Loading words file...".green());
    let words_file = File::open(path).expect("Cannot open words.txt containing the words to look for in the PDF");
    let words: HashSet<_> = BufReader::new(words_file)
        .lines()
        .map(|x| x.expect("Unable to parse a line from the words.txt"))
        .map(|word| word.to_lowercase())
        .collect();

    words
}

fn main() {
    let args = Args::parse();

    println!("{}", "Reading pdf...".green());
    let doc = Document::load(args.pdf).expect("Unable to open PDF");

    let words = args.words.map(|x| load_words(&x));

    println!("{}", "Extracting words from pages...".green());
    let mut index = BTreeMap::new();
    for (page_number, _) in doc.get_pages() {
        let text = doc.extract_text(&[page_number]).unwrap_or_else(|_| panic!("Unable to extract text from page {} from PDF", page_number));

        for word in text.split_whitespace() {
            let word = if !args.no_filtering {
                let word = word.trim_matches(|c| !char::is_alphabetic(c));
                if word.is_empty() { continue; }
                if !word.chars().all(char::is_alphabetic) { continue; }

                word
            }
            else { word };

            let key = word.to_lowercase();
            if match &words {
                Some(words) => !words.contains(&key),
                None => false,
            } {
                continue;
            }

            let entry = &mut *index.entry(key).or_insert(vec![]);
            entry.push((word.to_owned(), page_number));
        }
    }

    println!("{}", "Writing to output file...".green());
    let mut output = File::create(args.output).expect("Unable to create output file");
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
