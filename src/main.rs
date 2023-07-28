use std::collections::HashMap;

use itertools::Itertools;

use lopdf::{Document, Error};

fn main() -> Result<(), Error> {
    let doc = Document::load("thesis.pdf")?;

    let mut index: HashMap<String, Vec<u32>> = HashMap::new();
    for (page_number, _) in doc.get_pages() {
        let text = doc.extract_text(&[page_number])?;

        for word in text.split_whitespace() {
            let word = word.to_lowercase();

            let entry = &mut *index.entry(word).or_insert(vec![]);
            entry.push(page_number);
        }
    }

    for (word, pages) in index {
        let page_numbers = pages.iter().join(", ");
        println!("{}: {}", word, page_numbers);
    }

    Ok(())
}
