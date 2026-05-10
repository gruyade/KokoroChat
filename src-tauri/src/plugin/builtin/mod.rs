pub mod calculator;
pub mod file_ops;
pub mod web_search;

pub use calculator::CalculatorPlugin;
pub use file_ops::FileOpsPlugin;
pub use web_search::{SearchProvider, WebSearchConfig, WebSearchPlugin};
