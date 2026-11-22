pub(crate) const LOCAL_API_BASE: &str = "http://localhost:45731";

// Mensajes de commit genéricos: nunca incluir el package ni el nombre de la
// extensión. El historial del repo destino debe hablar de AEXT, no del
// contenido generado.
pub(crate) const COMMIT_MSG_GENERATE: &str = "AEXT: add generated extension";
pub(crate) const COMMIT_MSG_UPDATE: &str = "AEXT: update generated extension";
pub(crate) const COMMIT_MSG_DELETE: &str = "AEXT: remove generated extension";

#[cfg(test)]
mod tests {
    use super::{COMMIT_MSG_GENERATE, COMMIT_MSG_UPDATE};

    /// Regresión: los commits que AEXT escribe en el repo destino nunca deben
    /// llevar el package ni el nombre de la extensión.
    #[test]
    fn commit_messages_never_include_extension_identity() {
        for msg in [COMMIT_MSG_GENERATE, COMMIT_MSG_UPDATE] {
            let lower = msg.to_lowercase();
            assert!(
                !lower.contains('.'),
                "el mensaje parece incluir un package: {}",
                msg
            );
            assert!(
                !lower.contains("animeextension"),
                "el mensaje incluye un package de extensión: {}",
                msg
            );
            assert!(
                lower.starts_with("aext:"),
                "se esperaba un mensaje genérico de AEXT: {}",
                msg
            );
        }
    }
}
