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

        // Entity names are ASCII, but the text around them may not be: a 34-byte
        // window can end in the middle of a multi-byte character, and slicing
        // there panics. Back the limit up to the nearest character boundary
        // before slicing, the same way `scan_entity` does.
        let mut limit = (index + 34).min(bytes.len());
        while limit > index && !text.is_char_boundary(limit) {
            limit -= 1;
        }
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

    #[test]
    fn a_window_that_ends_inside_a_character_does_not_panic() {
        // A bare `&` followed by multi-byte text put byte 34 in the middle of a
        // character. Slicing there panicked; the window is backed up to the
        // boundary instead, so unremarkable text is left alone.
        let plain = format!("&{}", "é".repeat(17));
        assert_eq!(decode_entities(&plain), plain);

        // The same window shape must still decode a real entity in front of it.
        let decoded = format!("&amp;{}", "中".repeat(12));
        assert_eq!(decode_entities(&decoded), format!("&{}", "中".repeat(12)));
    }
}
