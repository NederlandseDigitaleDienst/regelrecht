#[cfg(feature = "annotation-validation")]
pub mod annotation_schema;
pub mod annotation_sidecar;
pub mod auth;
pub mod backend;
pub mod client;
pub mod config;
pub mod dto;
pub mod error;
#[cfg(feature = "github")]
pub mod github;
#[cfg(feature = "github")]
pub mod github_api_backend;
pub mod github_settings;
pub mod implements_index;
pub mod models;
pub mod registry;
pub mod source_map;
pub mod timing;
pub mod validation;

pub use backend::PrInfo;
#[cfg(feature = "github")]
pub use backend::SessionGitBackend;
pub use client::CorpusClient;
pub use config::{CorpusConfig, deployment_from_hostname};
pub use error::CorpusError;
#[cfg(feature = "github")]
pub use github::FetchResult;
#[cfg(feature = "github")]
pub use github_api_backend::GitHubApiBackend;
pub use github_settings::GithubSettings;
pub use models::{RegistryManifest, Source, SourceType};
pub use registry::{CorpusRegistry, ScanTokenOverride, SourceIndexFailure};
pub use source_map::SourceMap;
