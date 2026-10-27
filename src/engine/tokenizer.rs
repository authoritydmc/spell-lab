use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Token {
    pub raw: String,
    pub normalized: String,
    pub start_char: usize,
    pub end_char: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub acronym: bool,
}

/// Split text into word tokens while retaining both character and byte
/// offsets. The frontend uses character offsets; the correction API uses byte
/// offsets so UTF-8 text can be spliced safely.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current: Option<(usize, usize, usize, String)> = None; // byte, char, start char, value
    for (char_pos, (byte_pos, ch)) in text.char_indices().enumerate() {
        let word_char = ch.is_alphabetic() || ch == '\'' || ch == '-' || ch == '\u{2019}';
        if word_char {
            current
                .get_or_insert_with(|| (byte_pos, byte_pos, char_pos, String::new()))
                .3
                .push(ch);
        } else if let Some((start_byte, _, start_char, value)) = current.take() {
            push_token(&mut tokens, &value, start_byte, start_char);
        }
    }
    if let Some((start_byte, _, start_char, value)) = current.take() {
        push_token(&mut tokens, &value, start_byte, start_char);
    }
    tokens
}

fn push_token(tokens: &mut Vec<Token>, value: &str, start_byte: usize, start_char: usize) {
    let Some((first, _)) = value.char_indices().find(|(_, c)| c.is_alphabetic()) else {
        return;
    };
    let Some((last, last_char)) = value.char_indices().rev().find(|(_, c)| c.is_alphabetic())
    else {
        return;
    };
    let end_byte = last + last_char.len_utf8();
    if first >= end_byte {
        return;
    }
    let raw = &value[first..end_byte];
    let normalized = normalize(raw);
    if normalized.is_empty() {
        return;
    }
    let acronym = raw
        .chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase())
        && raw.chars().filter(|c| c.is_alphabetic()).count() >= 2;
    tokens.push(Token {
        raw: raw.to_string(),
        normalized,
        start_char: start_char + value[..first].chars().count(),
        end_char: start_char + value[..end_byte].chars().count(),
        start_byte: start_byte + value[..first].len(),
        end_byte: start_byte + end_byte,
        acronym,
    });
}

pub fn normalize(value: &str) -> String {
    value.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_and_offsets() {
        let text = "Teh  dog's! Well-known — fin.";
        let tokens = tokenize(text);
        let words: Vec<_> = tokens.iter().map(|t| t.raw.as_str()).collect();
        assert_eq!(words, vec!["Teh", "dog's", "Well-known", "fin"]);
        assert_eq!(tokens[0].start_char, 0);
        assert_eq!(tokens[0].end_char, 3);
        assert_eq!(tokens[1].start_byte, 5);
        assert_eq!(tokens[1].end_byte, 10);
        assert_eq!(tokens[2].normalized, "well-known");
    }

    #[test]
    fn trims_edge_punctuation() {
        let tokens = tokenize("'hello' --world--");
        let words: Vec<_> = tokens.iter().map(|t| t.raw.as_str()).collect();
        assert_eq!(words, vec!["hello", "world"]);
    }

    #[test]
    fn acronyms() {
        let tokens = tokenize("API and HTML5");
        assert!(tokens[0].acronym);
        assert!(!tokens[1].acronym);
        assert_eq!(tokens[2].raw, "HTML");
    }
}
