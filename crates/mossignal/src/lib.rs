//! Core value types for Mossignal.

pub mod authored;
mod compile;
pub mod diagnostics;
pub mod identity;
mod input;
pub mod key;
pub mod metadata;
pub mod signal;
pub mod time;

mod validation;

pub use compile::CompiledNetwork;
pub use identity::{InputSchemaFingerprint, NetworkFingerprint, TimeDomainId};
pub use input::{InputBuildFailure, InputSnapshot, InputSnapshotBuilder};
pub use validation::ValidatedNetwork;
