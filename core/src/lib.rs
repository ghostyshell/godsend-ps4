//! Framework-agnostic engine for managing a PS4 over GoldHEN's FTP, remote
//! package installer and virtual pad.
//!
//! Everything here works without a GUI so the same code backs the Tauri app,
//! the CLI and the MCP server.

pub mod aria2;
pub mod config;
pub mod convert;
pub mod debrid;
pub mod device;
pub mod doctor;
pub mod error;
pub mod fm;
pub mod ftp;
pub mod homebrew;
pub mod install;
pub mod library;
pub mod pad;
pub mod param;
pub mod pkg;
pub mod rpi;
pub mod saves;
pub mod sources;
pub mod split;
pub mod store;
pub mod transfer;

pub use config::{Config, Host};
pub use error::{Error, Result};
pub use library::GameEntry;
pub use transfer::{Job, JobManager, JobState};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Product name used for data directories and window titles.
pub const PRODUCT: &str = "GODsend PS4";
