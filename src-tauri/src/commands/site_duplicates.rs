use crate::models::ExtensionItem;

/// Comparación canónica de URLs de sitio para detectar fichas duplicadas.
/// No interpreta rutas internas: dos fichas del mismo sitio con distinto
/// idioma de paquete son el mismo sitio y no deben coexistir.
pub fn normalize_site_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    let without_scheme = lower
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(lower.as_str());
    let without_extras = without_scheme
        .split(|c| c == '?' || c == '#')
        .next()
        .unwrap_or(without_scheme);
    without_extras
        .trim_start_matches("www.")
        .trim_end_matches('/')
        .to_string()
}

/// Devuelve la ficha existente para el mismo sitio, si la hay. Se prefiere una
/// ficha activa sobre una que esté en la papelera para dar una explicación
/// útil al usuario.
pub fn find_existing_for_site<'a>(
    items: &'a [ExtensionItem],
    base_url: &str,
    ext_type: Option<&str>,
) -> Option<&'a ExtensionItem> {
    let site = normalize_site_url(base_url);
    if site.is_empty() {
        return None;
    }

    let mut trashed_match = None;
    for item in items {
        if let Some(expected) = ext_type {
            if !item.ext_type.eq_ignore_ascii_case(expected) {
                continue;
            }
        }
        if normalize_site_url(&item.base_url) != site {
            continue;
        }
        if !item.trashed {
            return Some(item);
        }
        if trashed_match.is_none() {
            trashed_match = Some(item);
        }
    }
    trashed_match
}

/// Mensaje genérico (sin lógica por extensión) para impedir una nueva variante.
pub fn duplicate_message(existing: &ExtensionItem) -> String {
    if existing.trashed {
        format!(
            "Ya existe una ficha en «Eliminadas» para este sitio ({}). Restaura esa ficha o elimínela definitivamente antes de generar otra.",
            existing.pkg_id
        )
    } else {
        format!(
            "Ya existe una extensión para este sitio ({}). Usa «Actualizar» en esa ficha en lugar de generar otra variante.",
            existing.pkg_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_site_url;

    #[test]
    fn normalizes_site_urls() {
        assert_eq!(
            normalize_site_url("https://WWW.Example.COM/path/?q=1#top"),
            "example.com/path"
        );
        assert_eq!(normalize_site_url("http://example.com/path/"), "example.com/path");
        assert_eq!(normalize_site_url(" example.com/path "), "example.com/path");
        assert!(normalize_site_url("  ").is_empty());
    }
}
