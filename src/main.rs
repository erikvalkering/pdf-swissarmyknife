use std::collections::HashSet;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, BufRead};

use clap::Parser;

use itertools::Itertools;

use lopdf::Document;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to PDF to generate an index for
    #[arg(short, long)]
    pdf: String,

    /// Path to file containing the words to be considered
    #[arg(short, long)]
    words: Option<String>,
}

fn load_words(path: &str) -> HashSet<String> {
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

    let doc = Document::load(args.pdf).expect("Unable to open PDF");

    let words = args.words.map(|x| load_words(&x));

    let mut index: BTreeMap<String, Vec<(String, u32)>> = BTreeMap::new();
    for (page_number, _) in doc.get_pages() {
        let text = doc.extract_text(&[page_number]).unwrap_or_else(|_| panic!("Unable to extract text from page {} from PDF", page_number));

        for word in text.split_whitespace() {
            let word = word.trim_matches(|c| "()[]".contains(c));
            if word.is_empty() { continue; }
            if !word.chars().all(char::is_alphabetic) { continue; }

            let key = word.to_lowercase();
            if match &words {
                Some(words) => !words.contains(&key),
                None => false,
            } {
                continue;
            }

            let entry: &mut Vec<(String, u32)> = &mut *index.entry(key).or_insert(vec![]);
            entry.push((word.to_owned(), page_number));
        }
    }

    for pages in index.values() {
        let unique_words: HashSet<_> = pages.iter().map(|(word, _)| word).collect();
        let page_numbers: HashSet<_> = pages.iter().map(|(_, page)| page).collect();

        println!(
            "{}: {}",
            unique_words.iter().join(", "),
            page_numbers.iter().join(", "),
        );
    }
}
