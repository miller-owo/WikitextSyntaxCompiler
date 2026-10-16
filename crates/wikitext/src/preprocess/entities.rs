pub fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    decode_entities_into(text, &mut out);
    out
}

pub fn decode_entities_into(text: &str, out: &mut String) {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    let mut start = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'&' {
            index += 1;
            continue;
        }

        let limit = (index + 34).min(bytes.len());
        let search = &text[index..limit];
        let decoded = search
            .find(';')
            .filter(|semicolon| *semicolon >= 2)
            .and_then(|semicolon| crate::entity::decode_at(&search[..=semicolon]));
        match decoded {
            Some((replacement, consumed)) => {
                out.push_str(&text[start..index]);
                out.push_str(&replacement);
                index += consumed;
                start = index;
            }
            None => index += 1,
        }
    }
    out.push_str(&text[start..]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leaves_plain_text_alone() {
        assert_eq!(decode_entities("plain"), "plain");
        assert_eq!(decode_entities("a & b"), "a & b");
    }

    #[test]
    fn decodes_named_and_numeric() {
        assert_eq!(decode_entities("&amp;"), "&");
        assert_eq!(decode_entities("&#65;&#x42;"), "AB");
        assert_eq!(decode_entities("&nbsp;"), "\u{a0}");
    }

    #[test]
    fn keeps_unknown_references() {
        assert_eq!(decode_entities("&notreal;"), "&notreal;");
        assert_eq!(decode_entities("100% & more"), "100% & more");
    }

    #[test]
    fn handles_multibyte_text() {
        assert_eq!(decode_entities("中文&amp;中文"), "中文&中文");
    }
}
