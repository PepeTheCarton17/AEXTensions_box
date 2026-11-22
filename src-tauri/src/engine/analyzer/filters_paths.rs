use scraper::{Html, Selector};
use std::collections::HashMap;

use super::models::{ApiOption, SiteAnalyzer, SitePathFilter};

/// Segmentos que identifican una taxonomía en la ruta de un enlace. La lista es
/// genérica: no hay dominios, idiomas ni rutas de un sitio concreto.
const PATH_SEGMENTS: &[&str] = &[
    "category", "categories", "genre", "genres", "tag", "tags", "country", "countries",
];

impl SiteAnalyzer {
    /// Filtros declarados como rutas de catálogo (`/.../category/<slug>/`).
    /// El texto visible del ancla es la etiqueta y la ruta completa es el valor:
    /// ambas cosas están en el HTML, así que no se inventa ninguna opción.
    /// Se exigen al menos dos opciones por grupo para no confundir un enlace
    /// suelto con un filtro del sitio.
    pub(crate) fn detect_path_filters(doc: &Html, origin: &str, host: &str) -> Vec<SitePathFilter> {
        let Ok(selector) = Selector::parse("a[href]") else {
            return Vec::new();
        };
        let mut groups: Vec<SitePathFilter> = Vec::new();

        for anchor in doc.select(&selector) {
            let label = anchor.text().collect::<String>().trim().to_string();
            if label.is_empty() || label.len() > 60 {
                continue;
            }
            let href = anchor.value().attr("href").unwrap_or("");
            let Some(path) = Self::same_origin_path(href, origin, host) else {
                continue;
            };
            if path.contains('?') || path.contains('#') {
                continue;
            }
            let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let Some((index, segment)) = segments
                .iter()
                .enumerate()
                .find(|(_, s)| PATH_SEGMENTS.contains(&s.to_lowercase().as_str()))
            else {
                continue;
            };
            if index + 1 >= segments.len() {
                continue;
            }
            let value = segments[index + 1];
            if value.is_empty()
                || value.len() > 60
                || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                continue;
            }

            let segment = segment.to_lowercase();
            let base_path = format!("/{}/", segments[..index].join("/"));
            let full_path = format!("/{}/", segments.join("/"));
            let position = groups
                .iter()
                .position(|g| g.base_path == base_path && g.segment == segment);
            let group = match position {
                Some(index) => &mut groups[index],
                None => {
                    if groups.len() >= 8 {
                        continue;
                    }
                    groups.push(SitePathFilter {
                        name: Self::canonical_filter_name(&segment),
                        segment,
                        base_path,
                        values: Vec::new(),
                    });
                    groups.last_mut().expect("grupo recién insertado")
                }
            };
            if group.values.len() >= 40 || group.values.iter().any(|v| v.value == full_path) {
                continue;
            }
            group.values.push(ApiOption {
                value: full_path,
                label,
            });
        }

        groups.retain(|g| g.values.len() >= 2);
        Self::qualify_duplicate_path_filter_names(&mut groups);
        groups
    }

    /// Varias secciones pueden publicar el mismo segmento (por ejemplo
    /// `/movies/category/` y `/shows/category/`). El último componente de la
    /// ruta real distingue los selectores sin inventar un nombre nuevo.
    fn qualify_duplicate_path_filter_names(groups: &mut [SitePathFilter]) {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for group in groups.iter() {
            *counts.entry(group.name.clone()).or_default() += 1;
        }
        for group in groups.iter_mut() {
            if counts.get(&group.name).copied().unwrap_or(0) < 2 {
                continue;
            }
            let qualifier = group
                .base_path
                .trim_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("")
                .replace(['-', '_'], " ");
            if qualifier.is_empty() {
                continue;
            }
            let mut chars = qualifier.chars();
            let qualifier = match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => continue,
            };
            group.name = format!("{} ({})", group.name, qualifier);
        }
    }
}
