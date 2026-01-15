//! Output formatting for CLI.

mod json;
mod text;

pub use json::JsonFormatter;
pub use text::TextFormatter;
#[cfg(test)]
mod tests;
