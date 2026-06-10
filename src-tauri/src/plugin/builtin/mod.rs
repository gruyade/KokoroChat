pub mod calculator;
pub mod file_ops;
pub mod knowledge;
pub mod web_search;

pub use calculator::CalculatorPlugin;
pub use file_ops::FileOpsPlugin;
pub use knowledge::KnowledgePlugin;
pub use web_search::{SearchProvider, WebSearchConfig, WebSearchPlugin};
