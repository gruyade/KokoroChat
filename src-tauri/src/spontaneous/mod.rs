pub mod speaker;

pub use speaker::{
    DefaultSpontaneousSpeaker, SpontaneousEvent, SpontaneousSpeaker, SpontaneousSpeakerConfig,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;
