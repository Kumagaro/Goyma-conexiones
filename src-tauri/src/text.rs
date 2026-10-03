fn collapse_space_sequences(input: &str) -> String {
    let mut collapsed = String::new();
    let mut previous_was_space = false;

    for ch in input.chars() {
        if ch.is_whitespace() {
            if !previous_was_space {
                collapsed.push(' ');
                previous_was_space = true;
            }
        } else {
            collapsed.push(ch);
            previous_was_space = false;
        }
    }

    collapsed.trim().to_owned()
}

pub fn clean_name(raw: &str) -> String {
    collapse_space_sequences(raw)
}

pub fn clean_description(raw: &str) -> Option<String> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = normalized
        .split('\n')
        .map(|line| collapse_space_sequences(line))
        .filter(|line| !line.is_empty())
        .collect();

    let joined = lines.join("\n");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

pub fn normalize_text(value: &str) -> String {
    let lowered = value.to_lowercase();
    let mut normalized = String::with_capacity(lowered.len());

    for ch in lowered.chars() {
        match ch {
            'á' => normalized.push('a'),
            'é' => normalized.push('e'),
            'í' => normalized.push('i'),
            'ó' => normalized.push('o'),
            'ú' => normalized.push('u'),
            'ü' => normalized.push('u'),
            _ => normalized.push(ch),
        }
    }

    collapse_space_sequences(&normalized)
}

pub fn spanish_sort_key(value: &str) -> String {
    let normalized = normalize_text(value);
    let mut key = String::with_capacity(normalized.len() + 4);

    for ch in normalized.chars() {
        if ch == 'ñ' {
            key.push('n');
            key.push(char::MAX);
        } else {
            key.push(ch);
        }
    }

    key
}

pub fn sort_spanish<T, K, F>(items: &mut [T], mut key_fn: F)
where
    F: FnMut(&T) -> K,
    K: Ord,
{
    items.sort_by_cached_key(|item| key_fn(item));
}

#[cfg(test)]
mod tests {
    use super::{clean_description, clean_name, normalize_text, sort_spanish, spanish_sort_key};

    #[test]
    fn clean_name_collapses_unicode_whitespace_and_trims() {
        assert_eq!(clean_name("  Camión\n\t  "), "Camión");
        assert_eq!(clean_name("  uno   dos\n\t tres  "), "uno dos tres");
        assert_eq!(clean_name("\r\n\t  x  \n  y  "), "x y");
    }

    #[test]
    fn clean_description_preserves_line_breaks_and_trims_each_line() {
        assert_eq!(
            clean_description("  Hola   mundo\n\t  otra\n\n  ").unwrap(),
            "Hola mundo\notra"
        );
        assert_eq!(clean_description("   \n\t  "), None);
    }

    #[test]
    fn normalize_text_handles_spanish_cases_without_nfc_nfd() {
        assert_eq!(normalize_text("Camión"), "camion");
        assert_eq!(normalize_text("CAMIÓN"), "camion");
        assert_eq!(normalize_text("camion"), "camion");
        assert_eq!(normalize_text("año"), "año");
        assert_eq!(normalize_text("ano"), "ano");
        assert_eq!(normalize_text("Ángel"), "angel");
        assert_eq!(normalize_text("  Café   con  ñ  "), "cafe con ñ");
        assert_ne!(normalize_text("año"), normalize_text("ano"));
    }

    #[test]
    fn spanish_sort_key_orders_n_and_ntilde_and_keeps_id_tiebreak() {
        assert!(spanish_sort_key("nuez") < spanish_sort_key("ñame"));
        assert!(spanish_sort_key("ñame") < spanish_sort_key("oso"));
        assert!(spanish_sort_key("n") < spanish_sort_key("ñ"));

        let mut items = vec![
            ("Ángel", 2),
            ("nuez", 5),
            ("ñame", 1),
            ("oso", 3),
        ];
        sort_spanish(&mut items, |item| (spanish_sort_key(item.0), item.1));
        assert_eq!(
            items.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            vec!["Ángel", "nuez", "ñame", "oso"]
        );
    }

    #[test]
    fn unicode_char_count_uses_codepoints_not_bytes() {
        assert_eq!(clean_name("ñ"), "ñ");
        assert_eq!(normalize_text("🙂"), "🙂");
        assert_eq!(clean_name("😀 a").chars().count(), 3);
    }
}
