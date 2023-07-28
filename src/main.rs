use lopdf::{Document};

fn extract_text_from_page(pdf_path: &str, page_number: u32) -> Result<String, Box<dyn std::error::Error>> {
    let doc = Document::load(pdf_path)?;

    let text = doc.extract_text(&[page_number])?;
    Ok(text)
}

fn main() {
    let out = extract_text_from_page("thesis.pdf", 1).unwrap();
    for word in out.split_whitespace() {
        println!("{}", word);
    }
}
