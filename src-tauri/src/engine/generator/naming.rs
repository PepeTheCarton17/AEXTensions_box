pub(crate) fn transliterate_char(c: char) -> Option<char> {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => Some('a'),
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => Some('e'),
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => Some('i'),
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => Some('o'),
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => Some('u'),
        'ñ' | 'Ñ' => Some('n'),
        'ç' | 'Ç' => Some('c'),
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => Some('y'),
        'æ' | 'Æ' => Some('a'),
        'ø' | 'Ø' => Some('o'),
        'ß' => Some('s'),
        _ => None,
    }
}

pub(crate) fn normalize_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch.is_alphabetic() {
            transliterate_char(ch)
        } else {
            None
        };

        match mapped {
            Some(c) if c.is_ascii_alphanumeric() => current.push(c),
            _ => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

pub(crate) fn sanitize_class_name(name: &str) -> String {
    let words = normalize_words(name);
    if words.is_empty() {
        return "CustomExtension".to_string();
    }

    let mut out = String::new();
    for word in words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }

    if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        out.insert_str(0, "Ext");
    }
    out
}

pub(crate) fn sanitize_pkg_slug(name: &str) -> String {
    let words = normalize_words(name);
    let slug = words
        .iter()
        .map(|w| w.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("");

    if slug.is_empty() {
        "customextension".to_string()
    } else if slug.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("ext{}", slug)
    } else {
        slug
    }
}

pub(crate) fn sanitize_display_name(name: &str) -> String {
    let words = normalize_words(name);
    if words.is_empty() {
        return "Custom Extension".to_string();
    }
    words
        .iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => {
                    let mut s = String::new();
                    s.push(first.to_ascii_uppercase());
                    s.push_str(&chars.as_str().to_ascii_lowercase());
                    s
                }
                None => String::new(),
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn kt_path_literal(value: Option<&str>) -> String {
    value
        .unwrap_or("")
        .trim()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r', '\t'], " ")
}
