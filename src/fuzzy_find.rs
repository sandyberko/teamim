const SEARCH_CHUNK: usize = 32;

#[must_use]
pub fn find(src: &str, snippet: &str) -> Option<usize> {
    let snippet = if let Some((idx, _)) = snippet.char_indices().nth(SEARCH_CHUNK + 1) {
        &snippet[..idx]
    } else {
        snippet
    };
    src.char_indices().position(|(text_byte_idx, _)| {
        let mut text_chars = src[text_byte_idx..].chars();
        let mut snippet_chars = snippet.chars();

        for _ in 0..SEARCH_CHUNK {
            match (text_chars.next(), snippet_chars.next()) {
                (Some(text_c), Some(snippet_c)) if text_c == snippet_c => continue, // Match, keep going
                _ => return false, // Mismatch or end of string
            }
        }
        true // Full snippet matched
    })
}
