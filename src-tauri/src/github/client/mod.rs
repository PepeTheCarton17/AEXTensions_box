//! Cliente HTTP de GitHub.
//!
//! El antiguo archivo único se reparte por áreas (modelos, peticiones base,
//! repositorios, contenidos, commits, borrado de módulos y Actions). Las
//! firmas públicas y los re-exports son idénticos a los del archivo original.

mod commits;
mod contents;
mod delete;
mod models;
mod repos;
mod request;
mod workflows;

pub use models::{CreateRepoParams, GithubApiClient, RepoInfo, RepoSummary, WorkflowRun};
