//! Registro genérico de motores: descriptores, manifiestos, instalación,
//! enrutado por capacidad y sondas anónimas.

mod builtin;
mod install;
mod manifest;
mod model;
mod probe;
mod router;
mod runtime;
mod store;

pub use install::{fetch, save_files, RepoRef};
pub use manifest::parse;
pub use model::{EngineDescriptor, EngineKind, EngineProbe, EngineSource, EngineTrial};
pub use probe::{probe_engine, ProbeContext};
pub use router::select;
pub use runtime::{EngineRuntime, STREAM_RESOLVER_API};
pub use store::{disable, engines_path, install_dir, load, persist, upsert, ENGINES_DIR, ENGINES_FILE};
