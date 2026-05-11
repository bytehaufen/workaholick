mod app;
mod bootstrap;
pub mod cli;
pub mod config;
mod domain;
mod editor;
mod store;

pub use crate::app::App;
pub use crate::app::DayOverview;
pub use crate::bootstrap::run_app;
pub use crate::store::JsonlFileStore;
pub use crate::store::RecordReader;
pub use crate::store::RecordStore;
pub use crate::store::RecordWriter;

#[cfg(test)]
/// Shared test helpers
mod test_support;
