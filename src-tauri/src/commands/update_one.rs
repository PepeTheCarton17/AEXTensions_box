//! Compatibilidad de la actualización individual con el pipeline por lotes.

use crate::commands::update_prepare::{prepare_update, resolve_update_lang};
use crate::commands::update_publish::publish_updates;
use crate::models::ExtensionItem;
use crate::state::AppState;

/// Actualiza una extensión usando el mismo pipeline que el resto de comandos:
/// preparar primero, publicar después. La publicación agrupa por repo, así que
 /// una actualización individual también pasa por un único commit.
pub(crate) async fn update_extension_inner(
    pkg_id: String,
    requested_lang: Option<String>,
    state: &AppState,
) -> Result<ExtensionItem, String> {
    let prepared = prepare_update(&pkg_id, requested_lang.as_deref(), state).await?;
    let mut results = publish_updates(vec![prepared], state).await;
    results
        .pop()
        .ok_or_else(|| format!("No se pudo confirmar la actualización de {}", pkg_id))
}

#[cfg(test)]
mod tests {
    use super::resolve_update_lang;

    #[test]
    fn chosen_language_only_affects_labels_on_update() {
        assert_eq!(resolve_update_lang(Some("es"), "all"), "es");
        assert_eq!(resolve_update_lang(None, "en"), "en");
    }
}
