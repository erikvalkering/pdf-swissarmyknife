use std::collections::HashSet;
use std::collections::HashMap;

use itertools::Itertools;

use lopdf::{Document, Error};

fn main() -> Result<(), Error> {
    let doc = Document::load("thesis.pdf")?;

    let words: HashSet<_> = vec!["nurbs".to_owned(), "ray".to_owned(), "erik".to_owned()].into_iter().collect();

    let mut index: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    for (page_number, _) in doc.get_pages() {
        let text = doc.extract_text(&[page_number])?;

        for word in text.split_whitespace() {
            let key = word.to_lowercase();
            if words.contains(&key) {
                let entry = &mut *index.entry(key).or_insert(vec![]);
                entry.push((word.to_owned(), page_number));
            }
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

    Ok(())
}
