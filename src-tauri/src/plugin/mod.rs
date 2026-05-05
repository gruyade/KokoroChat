pub mod builtin;
pub mod registry;
pub mod system;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;

pub use registry::{DefaultPluginRegistry, PluginRegistry};
pub use system::{DefaultPluginSystem, PluginHandler, PluginSystem};
