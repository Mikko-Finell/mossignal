//! Core value types for Mossignal.

pub mod authored;
mod compile;
pub mod diagnostics;
pub mod identity;
mod input;
pub mod key;
mod machine;
pub mod metadata;
mod policy;
pub mod signal;
pub mod time;

mod validation;

pub use compile::CompiledNetwork;
pub use identity::{InputSchemaFingerprint, NetworkFingerprint, TimeDomainId};
pub use input::{InputBuildFailure, InputSnapshot, InputSnapshotBuilder};
pub use machine::{Machine, MachineStatus, NetworkRevision};
pub use policy::{
    PolicyFailure, RuntimePolicy, RuntimePolicyBuilder, RuntimePolicyId, RuntimePolicyLimit,
};
pub use validation::ValidatedNetwork;
