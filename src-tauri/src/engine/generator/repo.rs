use super::templates::repo_index::{repo_readme_md_fmt, REPO_INDEX_PY};
use super::templates::workflow::WORKFLOW_YML;

pub(crate) fn generate_github_workflow() -> String {
    WORKFLOW_YML.to_string()
}

pub(crate) fn generate_repo_index_script() -> String {
    REPO_INDEX_PY.to_string()
}

/// README mínimo y estable para repositorios creados por AEXT.
///
/// No incluye el nombre de la extensión: el mismo contenido sirve para un repo
/// con una o con veinte extensiones y no cambia en cada generación. Si el repo
/// ya tiene un README propio, AEXT lo respeta y no usa esta plantilla.
pub fn generate_repo_readme(owner: &str, repo: &str, html_url: &str) -> String {
    let install_url = format!(
        "https://raw.githubusercontent.com/{}/{}/gh-pages/index.min.json",
        owner, repo
    );
    format!(
        concat!(repo_readme_md_fmt!()),
        install_url = install_url,
        html_url = html_url,
    )
}
