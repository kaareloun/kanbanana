//! SQLite cache, workspaces, dev state, and API keys in a private credentials file.
//! The GUI and the MCP server open the same file from two processes.

mod catalog;
mod comments;
mod db;
mod dev;
mod error;
mod issues;
mod legacy;
mod migrate;
pub mod secrets;
mod workspaces;

pub use catalog::*;
pub use comments::*;
pub use db::{Db, data_version, default_path, open_at, open_default};
pub use dev::*;
pub use error::{Error, Result};
pub use issues::*;
pub use legacy::move_old_folders;
pub use secrets::{delete_api_key, get_api_key, set_api_key};
pub use workspaces::*;

#[cfg(test)]
mod tests;
