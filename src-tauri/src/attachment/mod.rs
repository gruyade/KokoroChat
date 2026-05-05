pub mod processor;

pub use processor::{AttachmentProcessor, DefaultAttachmentProcessor};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;
