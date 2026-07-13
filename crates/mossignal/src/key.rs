//! Durable structural identity values.
//!
//! Structural keys identify authored network elements independently of dense,
//! revision-local runtime positions. Their opaque numeric payloads may be
//! reconstructed deliberately, but numeric order has no semantic meaning and
//! equal payloads in different key categories remain distinct identities.
//!
//! Structural categories cannot be substituted for one another:
//!
//! ```compile_fail
//! use mossignal::key::{ConnectionKey, NodeKey};
//!
//! fn accepts_node(_: NodeKey) {}
//!
//! accepts_node(ConnectionKey::from_u128(1));
//! ```
//!
//! Signal kind and port direction are also part of the type:
//!
//! ```compile_fail
//! use mossignal::key::InPortKey;
//! use mossignal::signal::{Level, Pulse};
//!
//! fn accepts_level(_: InPortKey<Level>) {}
//!
//! accepts_level(InPortKey::<Pulse>::from_u128(1));
//! ```
//!
//! ```compile_fail
//! use mossignal::key::{InPortKey, OutPortKey};
//! use mossignal::signal::Level;
//!
//! fn accepts_input(_: InPortKey<Level>) {}
//!
//! accepts_input(OutPortKey::<Level>::from_u128(1));
//! ```
//!
//! Only Mossignal's sealed signal markers can parameterize typed keys:
//!
//! ```compile_fail
//! use mossignal::key::InPortKey;
//!
//! struct CustomSignal;
//!
//! let _ = InPortKey::<CustomSignal>::from_u128(1);
//! ```
//!
//! A [`KeyAllocator`] cannot be cloned because duplicated allocator state could
//! issue duplicate values:
//!
//! ```compile_fail
//! use mossignal::key::KeyAllocator;
//!
//! let allocator = KeyAllocator::new(1);
//! let _duplicate = allocator.clone();
//! ```

use crate::signal::{Level, Pulse, SignalKind, SignalType};
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

const KIND_LEVEL: u8 = 0;
const KIND_PULSE: u8 = 1;
const SOURCE_EXTERNAL_INPUT: u8 = 0;
const SOURCE_OUT_PORT: u8 = 1;

const DOMAIN_NETWORK: u8 = 1;
const DOMAIN_NODE: u8 = 2;
const DOMAIN_CONNECTION: u8 = 3;
const DOMAIN_MODULE_INSTANCE: u8 = 4;
const DOMAIN_MODULE_INPUT: u8 = 5;
const DOMAIN_MODULE_OUTPUT: u8 = 6;
const DOMAIN_IN_PORT: u8 = 7;
const DOMAIN_OUT_PORT: u8 = 8;
const DOMAIN_EXTERNAL_INPUT: u8 = 9;
const DOMAIN_EXTERNAL_OUTPUT: u8 = 10;
const DOMAIN_SIGNAL_SOURCE: u8 = 11;
const DOMAIN_ANY_MODULE_INPUT: u8 = 12;
const DOMAIN_ANY_MODULE_OUTPUT: u8 = 13;
const DOMAIN_ANY_IN_PORT: u8 = 14;
const DOMAIN_ANY_OUT_PORT: u8 = 15;
const DOMAIN_ANY_EXTERNAL_INPUT: u8 = 16;
const DOMAIN_ANY_EXTERNAL_OUTPUT: u8 = 17;
const DOMAIN_ANY_SIGNAL_SOURCE: u8 = 18;

fn hash_kind<H: Hasher>(kind: SignalKind, state: &mut H) {
    state.write_u8(match kind {
        SignalKind::Level => KIND_LEVEL,
        SignalKind::Pulse => KIND_PULSE,
    });
}

macro_rules! direct_key {
    ($name:ident, $domain:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// Every `u128` payload is valid. Ordering compares only that payload
        /// and exists for deterministic canonicalization, not to express
        /// creation, topology, priority, or ownership.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(u128);

        impl $name {
            /// Reconstructs this structural identity from its complete durable
            /// opaque representation.
            #[must_use]
            pub const fn from_u128(value: u128) -> Self {
                Self(value)
            }

            /// Returns the exact opaque representation of this identity.
            #[must_use]
            pub const fn as_u128(self) -> u128 {
                self.0
            }
        }

        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                // SPEC: docs/specs/concrete_rust_api_surface.md §16 "Stable keys"
                // Structural category remains part of identity for equal payloads.
                state.write_u8($domain);
                state.write_u128(self.0);
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

direct_key!(
    NetworkKey,
    DOMAIN_NETWORK,
    "The durable authored identity of a network."
);
direct_key!(
    NodeKey,
    DOMAIN_NODE,
    "The durable authored identity of a node."
);
direct_key!(
    ConnectionKey,
    DOMAIN_CONNECTION,
    "The durable authored identity of a connection."
);
direct_key!(
    ModuleInstanceKey,
    DOMAIN_MODULE_INSTANCE,
    "The durable authored identity of a module instance."
);

macro_rules! typed_key {
    ($name:ident, $domain:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// `S` preserves whether this identity carries a level or pulse signal.
        /// Every `u128` payload is valid. Ordering compares only payloads within
        /// the same typed key category and has no semantic meaning.
        pub struct $name<S: SignalType> {
            value: u128,
            marker: PhantomData<fn() -> S>,
        }

        impl<S: SignalType> $name<S> {
            /// Reconstructs this typed structural identity from its complete
            /// durable opaque representation.
            #[must_use]
            pub const fn from_u128(value: u128) -> Self {
                Self {
                    value,
                    marker: PhantomData,
                }
            }

            /// Returns the exact opaque representation of this typed identity.
            #[must_use]
            pub const fn as_u128(self) -> u128 {
                self.value
            }
        }

        impl<S: SignalType> Clone for $name<S> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<S: SignalType> Copy for $name<S> {}

        impl<S: SignalType> PartialEq for $name<S> {
            fn eq(&self, other: &Self) -> bool {
                self.value == other.value
            }
        }

        impl<S: SignalType> Eq for $name<S> {}

        impl<S: SignalType> PartialOrd for $name<S> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<S: SignalType> Ord for $name<S> {
            fn cmp(&self, other: &Self) -> Ordering {
                self.value.cmp(&other.value)
            }
        }

        impl<S: SignalType> Hash for $name<S> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                // SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md
                // §8 "Ordinary stable-key preservation"
                // Signal kind cannot change while an equal payload survives.
                state.write_u8($domain);
                hash_kind(S::KIND, state);
                state.write_u128(self.value);
            }
        }

        impl<S: SignalType> fmt::Debug for $name<S> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("kind", &S::KIND)
                    .field("value", &self.value)
                    .finish()
            }
        }
    };
}

typed_key!(
    ModuleInputKey,
    DOMAIN_MODULE_INPUT,
    "The durable authored identity of a typed module input."
);
typed_key!(
    ModuleOutputKey,
    DOMAIN_MODULE_OUTPUT,
    "The durable authored identity of a typed module output."
);
typed_key!(
    InPortKey,
    DOMAIN_IN_PORT,
    "The durable authored identity of a typed node input port."
);
typed_key!(
    OutPortKey,
    DOMAIN_OUT_PORT,
    "The durable authored identity of a typed node output port."
);
typed_key!(
    ExternalInputKey,
    DOMAIN_EXTERNAL_INPUT,
    "The durable authored identity of a typed external input."
);
typed_key!(
    ExternalOutputKey,
    DOMAIN_EXTERNAL_OUTPUT,
    "The durable authored identity of a typed external output."
);

/// A typed network signal source.
///
/// The outer type retains signal kind while the variant retains whether the
/// source is an external input or an output port. Ordering places external
/// inputs before output ports, then compares payloads; this is canonical order
/// only and conveys no semantic precedence.
#[non_exhaustive]
pub enum SignalSourceKey<S: SignalType> {
    /// A signal supplied through a typed external input.
    ExternalInput(ExternalInputKey<S>),
    /// A signal produced by a typed node output port.
    OutPort(OutPortKey<S>),
}

impl<S: SignalType> Clone for SignalSourceKey<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: SignalType> Copy for SignalSourceKey<S> {}

impl<S: SignalType> PartialEq for SignalSourceKey<S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ExternalInput(left), Self::ExternalInput(right)) => left == right,
            (Self::OutPort(left), Self::OutPort(right)) => left == right,
            _ => false,
        }
    }
}

impl<S: SignalType> Eq for SignalSourceKey<S> {}

impl<S: SignalType> PartialOrd for SignalSourceKey<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: SignalType> Ord for SignalSourceKey<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::ExternalInput(left), Self::ExternalInput(right)) => left.cmp(right),
            (Self::ExternalInput(_), Self::OutPort(_)) => Ordering::Less,
            (Self::OutPort(_), Self::ExternalInput(_)) => Ordering::Greater,
            (Self::OutPort(left), Self::OutPort(right)) => left.cmp(right),
        }
    }
}

impl<S: SignalType> Hash for SignalSourceKey<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(DOMAIN_SIGNAL_SOURCE);
        hash_kind(S::KIND, state);
        match self {
            Self::ExternalInput(key) => {
                state.write_u8(SOURCE_EXTERNAL_INPUT);
                state.write_u128(key.value);
            }
            Self::OutPort(key) => {
                state.write_u8(SOURCE_OUT_PORT);
                state.write_u128(key.value);
            }
        }
    }
}

impl<S: SignalType> fmt::Debug for SignalSourceKey<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExternalInput(key) => formatter
                .debug_tuple("SignalSourceKey::ExternalInput")
                .field(key)
                .finish(),
            Self::OutPort(key) => formatter
                .debug_tuple("SignalSourceKey::OutPort")
                .field(key)
                .finish(),
        }
    }
}

impl<S: SignalType> From<ExternalInputKey<S>> for SignalSourceKey<S> {
    fn from(key: ExternalInputKey<S>) -> Self {
        Self::ExternalInput(key)
    }
}

impl<S: SignalType> From<OutPortKey<S>> for SignalSourceKey<S> {
    fn from(key: OutPortKey<S>) -> Self {
        Self::OutPort(key)
    }
}

macro_rules! erased_key {
    ($any:ident, $typed:ident, $domain:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// Erasure removes the compile-time signal marker but retains the kind
        /// in the enum variant. Level values sort before pulse values regardless
        /// of payload; numeric ordering remains non-semantic.
        #[non_exhaustive]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $any {
            /// A level-signal key.
            Level($typed<Level>),
            /// A pulse-signal key.
            Pulse($typed<Pulse>),
        }

        impl $any {
            /// Returns the signal kind retained by this erased key.
            #[must_use]
            pub const fn kind(self) -> SignalKind {
                match self {
                    Self::Level(_) => SignalKind::Level,
                    Self::Pulse(_) => SignalKind::Pulse,
                }
            }
        }

        impl PartialOrd for $any {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $any {
            fn cmp(&self, other: &Self) -> Ordering {
                // SPEC: docs/specs/concrete_rust_api_surface.md §18 "Erased stable keys"
                // Kind order is explicit rather than an enum-discriminant accident.
                match (self, other) {
                    (Self::Level(left), Self::Level(right)) => left.cmp(right),
                    (Self::Level(_), Self::Pulse(_)) => Ordering::Less,
                    (Self::Pulse(_), Self::Level(_)) => Ordering::Greater,
                    (Self::Pulse(left), Self::Pulse(right)) => left.cmp(right),
                }
            }
        }

        impl Hash for $any {
            fn hash<H: Hasher>(&self, state: &mut H) {
                state.write_u8($domain);
                match self {
                    Self::Level(key) => {
                        hash_kind(SignalKind::Level, state);
                        state.write_u128(key.value);
                    }
                    Self::Pulse(key) => {
                        hash_kind(SignalKind::Pulse, state);
                        state.write_u128(key.value);
                    }
                }
            }
        }

        impl From<$typed<Level>> for $any {
            fn from(key: $typed<Level>) -> Self {
                Self::Level(key)
            }
        }

        impl From<$typed<Pulse>> for $any {
            fn from(key: $typed<Pulse>) -> Self {
                Self::Pulse(key)
            }
        }

        impl TryFrom<$any> for $typed<Level> {
            type Error = KeyKindMismatch;

            fn try_from(key: $any) -> Result<Self, Self::Error> {
                match key {
                    $any::Level(key) => Ok(key),
                    $any::Pulse(_) => {
                        Err(KeyKindMismatch::new(SignalKind::Level, SignalKind::Pulse))
                    }
                }
            }
        }

        impl TryFrom<$any> for $typed<Pulse> {
            type Error = KeyKindMismatch;

            fn try_from(key: $any) -> Result<Self, Self::Error> {
                match key {
                    $any::Pulse(key) => Ok(key),
                    $any::Level(_) => {
                        Err(KeyKindMismatch::new(SignalKind::Pulse, SignalKind::Level))
                    }
                }
            }
        }
    };
}

erased_key!(
    AnyModuleInputKey,
    ModuleInputKey,
    DOMAIN_ANY_MODULE_INPUT,
    "A signal-kind-erased module-input identity."
);
erased_key!(
    AnyModuleOutputKey,
    ModuleOutputKey,
    DOMAIN_ANY_MODULE_OUTPUT,
    "A signal-kind-erased module-output identity."
);
erased_key!(
    AnyInPortKey,
    InPortKey,
    DOMAIN_ANY_IN_PORT,
    "A signal-kind-erased node input-port identity."
);
erased_key!(
    AnyOutPortKey,
    OutPortKey,
    DOMAIN_ANY_OUT_PORT,
    "A signal-kind-erased node output-port identity."
);
erased_key!(
    AnyExternalInputKey,
    ExternalInputKey,
    DOMAIN_ANY_EXTERNAL_INPUT,
    "A signal-kind-erased external-input identity."
);
erased_key!(
    AnyExternalOutputKey,
    ExternalOutputKey,
    DOMAIN_ANY_EXTERNAL_OUTPUT,
    "A signal-kind-erased external-output identity."
);

/// A signal-kind-erased network signal source.
///
/// The outer variant retains level versus pulse, and the nested source variant
/// retains external input versus output port. Canonical ordering compares kind,
/// then source category, then payload; none of those orderings is semantic.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnySignalSourceKey {
    /// A level-signal source.
    Level(SignalSourceKey<Level>),
    /// A pulse-signal source.
    Pulse(SignalSourceKey<Pulse>),
}

impl AnySignalSourceKey {
    /// Returns the signal kind retained by this erased source.
    #[must_use]
    pub const fn kind(self) -> SignalKind {
        match self {
            Self::Level(_) => SignalKind::Level,
            Self::Pulse(_) => SignalKind::Pulse,
        }
    }
}

impl PartialOrd for AnySignalSourceKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AnySignalSourceKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Level(left), Self::Level(right)) => left.cmp(right),
            (Self::Level(_), Self::Pulse(_)) => Ordering::Less,
            (Self::Pulse(_), Self::Level(_)) => Ordering::Greater,
            (Self::Pulse(left), Self::Pulse(right)) => left.cmp(right),
        }
    }
}

impl Hash for AnySignalSourceKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(DOMAIN_ANY_SIGNAL_SOURCE);
        match self {
            Self::Level(source) => hash_erased_source(SignalKind::Level, *source, state),
            Self::Pulse(source) => hash_erased_source(SignalKind::Pulse, *source, state),
        }
    }
}

fn hash_erased_source<S: SignalType, H: Hasher>(
    kind: SignalKind,
    source: SignalSourceKey<S>,
    state: &mut H,
) {
    hash_kind(kind, state);
    match source {
        SignalSourceKey::ExternalInput(key) => {
            state.write_u8(SOURCE_EXTERNAL_INPUT);
            state.write_u128(key.value);
        }
        SignalSourceKey::OutPort(key) => {
            state.write_u8(SOURCE_OUT_PORT);
            state.write_u128(key.value);
        }
    }
}

impl From<SignalSourceKey<Level>> for AnySignalSourceKey {
    fn from(key: SignalSourceKey<Level>) -> Self {
        Self::Level(key)
    }
}

impl From<SignalSourceKey<Pulse>> for AnySignalSourceKey {
    fn from(key: SignalSourceKey<Pulse>) -> Self {
        Self::Pulse(key)
    }
}

impl TryFrom<AnySignalSourceKey> for SignalSourceKey<Level> {
    type Error = KeyKindMismatch;

    fn try_from(key: AnySignalSourceKey) -> Result<Self, Self::Error> {
        match key {
            AnySignalSourceKey::Level(key) => Ok(key),
            AnySignalSourceKey::Pulse(_) => {
                Err(KeyKindMismatch::new(SignalKind::Level, SignalKind::Pulse))
            }
        }
    }
}

impl TryFrom<AnySignalSourceKey> for SignalSourceKey<Pulse> {
    type Error = KeyKindMismatch;

    fn try_from(key: AnySignalSourceKey) -> Result<Self, Self::Error> {
        match key {
            AnySignalSourceKey::Pulse(key) => Ok(key),
            AnySignalSourceKey::Level(_) => {
                Err(KeyKindMismatch::new(SignalKind::Pulse, SignalKind::Level))
            }
        }
    }
}

/// A checked erased-to-typed conversion failure.
///
/// The error retains both signal kinds so callers need not parse display text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyKindMismatch {
    expected: SignalKind,
    actual: SignalKind,
}

impl KeyKindMismatch {
    const fn new(expected: SignalKind, actual: SignalKind) -> Self {
        Self { expected, actual }
    }

    /// Returns the signal kind required by the requested typed key.
    #[must_use]
    pub const fn expected(self) -> SignalKind {
        self.expected
    }

    /// Returns the signal kind carried by the erased key.
    #[must_use]
    pub const fn actual(self) -> SignalKind {
        self.actual
    }
}

impl fmt::Display for KeyKindMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "expected {:?} signal key, found {:?} signal key",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for KeyKindMismatch {}

#[derive(Debug)]
struct Counter {
    next: u64,
    exhausted: bool,
}

impl Counter {
    const ZERO: Self = Self {
        next: 0,
        exhausted: false,
    };

    fn allocate(&mut self) -> u64 {
        assert!(
            !self.exhausted,
            "structural key allocation domain exhausted after issuing every u64 counter"
        );

        let allocated = self.next;
        if allocated == u64::MAX {
            self.exhausted = true;
        } else {
            self.next = allocated + 1;
        }
        allocated
    }
}

#[derive(Debug)]
struct KindCounters {
    level: Counter,
    pulse: Counter,
}

impl KindCounters {
    const ZERO: Self = Self {
        level: Counter::ZERO,
        pulse: Counter::ZERO,
    };

    fn for_kind(&mut self, kind: SignalKind) -> &mut Counter {
        match kind {
            SignalKind::Level => &mut self.level,
            SignalKind::Pulse => &mut self.pulse,
        }
    }
}

/// A deterministic caller-owned allocator for structural keys.
///
/// The caller coordinates the namespace; this allocator is not a global
/// uniqueness service. Each structural category and signal kind has an
/// independent counter, so separate domains intentionally may produce equal
/// numeric payloads. Keys are not validated against a network by allocation.
/// The allocator cannot be cloned, copied, or default-constructed.
///
/// # Panics
///
/// An allocation method panics only after its domain has issued every counter
/// in `0..=u64::MAX`. Exhausting one domain does not affect any other domain.
#[derive(Debug)]
pub struct KeyAllocator {
    namespace: u64,
    network: Counter,
    node: Counter,
    connection: Counter,
    module_instance: Counter,
    module_input: KindCounters,
    module_output: KindCounters,
    in_port: KindCounters,
    out_port: KindCounters,
    external_input: KindCounters,
    external_output: KindCounters,
}

impl KeyAllocator {
    /// Creates an allocator in a caller-coordinated namespace.
    ///
    /// No namespace value is reserved. Allocation begins at counter zero in
    /// every independent domain.
    #[must_use]
    pub const fn new(namespace: u64) -> Self {
        Self {
            namespace,
            network: Counter::ZERO,
            node: Counter::ZERO,
            connection: Counter::ZERO,
            module_instance: Counter::ZERO,
            module_input: KindCounters::ZERO,
            module_output: KindCounters::ZERO,
            in_port: KindCounters::ZERO,
            out_port: KindCounters::ZERO,
            external_input: KindCounters::ZERO,
            external_output: KindCounters::ZERO,
        }
    }

    fn payload(&self, counter: u64) -> u128 {
        // SPEC: docs/specs/concrete_rust_api_surface.md §17 "Key construction"
        // Caller namespace and domain-local counter determine allocation exactly.
        (u128::from(self.namespace) << 64) | u128::from(counter)
    }

    /// Allocates the next network identity in this allocator's network domain.
    pub fn network(&mut self) -> NetworkKey {
        let counter = self.network.allocate();
        NetworkKey::from_u128(self.payload(counter))
    }

    /// Allocates the next node identity in this allocator's node domain.
    pub fn node(&mut self) -> NodeKey {
        let counter = self.node.allocate();
        NodeKey::from_u128(self.payload(counter))
    }

    /// Allocates the next connection identity in its independent domain.
    pub fn connection(&mut self) -> ConnectionKey {
        let counter = self.connection.allocate();
        ConnectionKey::from_u128(self.payload(counter))
    }

    /// Allocates the next module-instance identity in its independent domain.
    pub fn module_instance(&mut self) -> ModuleInstanceKey {
        let counter = self.module_instance.allocate();
        ModuleInstanceKey::from_u128(self.payload(counter))
    }

    /// Allocates the next typed module-input identity for signal kind `S`.
    pub fn module_input<S: SignalType>(&mut self) -> ModuleInputKey<S> {
        let counter = self.module_input.for_kind(S::KIND).allocate();
        ModuleInputKey::from_u128(self.payload(counter))
    }

    /// Allocates the next typed module-output identity for signal kind `S`.
    pub fn module_output<S: SignalType>(&mut self) -> ModuleOutputKey<S> {
        let counter = self.module_output.for_kind(S::KIND).allocate();
        ModuleOutputKey::from_u128(self.payload(counter))
    }

    /// Allocates the next typed input-port identity for signal kind `S`.
    pub fn in_port<S: SignalType>(&mut self) -> InPortKey<S> {
        let counter = self.in_port.for_kind(S::KIND).allocate();
        InPortKey::from_u128(self.payload(counter))
    }

    /// Allocates the next typed output-port identity for signal kind `S`.
    pub fn out_port<S: SignalType>(&mut self) -> OutPortKey<S> {
        let counter = self.out_port.for_kind(S::KIND).allocate();
        OutPortKey::from_u128(self.payload(counter))
    }

    /// Allocates the next typed external-input identity for signal kind `S`.
    pub fn external_input<S: SignalType>(&mut self) -> ExternalInputKey<S> {
        let counter = self.external_input.for_kind(S::KIND).allocate();
        ExternalInputKey::from_u128(self.payload(counter))
    }

    /// Allocates the next typed external-output identity for signal kind `S`.
    pub fn external_output<S: SignalType>(&mut self) -> ExternalOutputKey<S> {
        let counter = self.external_output.for_kind(S::KIND).allocate();
        ExternalOutputKey::from_u128(self.payload(counter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::TraitHostileSignal;
    use core::fmt::Debug;
    use core::hash::Hash;
    use core::mem::size_of;
    use std::collections::HashSet;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    const MIXED: u128 = 0xa5a5_5a5a_f0f0_0f0f_1234_5678_9abc_def0;
    const CONST_NODE: NodeKey = NodeKey::from_u128(7);
    const CONST_IN_PORT: InPortKey<Level> = InPortKey::from_u128(9);
    const CONST_ALLOCATOR: KeyAllocator = KeyAllocator::new(11);

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum RecordedWrite {
        Bytes(Vec<u8>),
        U8(u8),
        U128(u128),
    }

    #[derive(Default)]
    struct RecordingHasher {
        writes: Vec<RecordedWrite>,
    }

    impl Hasher for RecordingHasher {
        fn finish(&self) -> u64 {
            0
        }

        fn write(&mut self, bytes: &[u8]) {
            self.writes.push(RecordedWrite::Bytes(bytes.to_vec()));
        }

        fn write_u8(&mut self, value: u8) {
            self.writes.push(RecordedWrite::U8(value));
        }

        fn write_u128(&mut self, value: u128) {
            self.writes.push(RecordedWrite::U128(value));
        }
    }

    fn hash_input<T: Hash>(value: &T) -> Vec<RecordedWrite> {
        let mut hasher = RecordingHasher::default();
        value.hash(&mut hasher);
        hasher.writes
    }

    fn assert_value_traits<T: Debug + Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Hash>() {}

    #[allow(clippy::clone_on_copy)]
    fn verify_direct_family<T>(
        family: &str,
        from_u128: impl Fn(u128) -> T,
        as_u128: impl Fn(T) -> u128,
    ) where
        T: Debug + Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Hash,
    {
        let values = [0, 1, MIXED, u128::MAX];
        let mut keys = Vec::new();

        for value in values {
            let key = from_u128(value);
            assert_eq!(as_u128(key), value, "{family} round trip for {value}");
            assert_eq!(key, from_u128(value), "{family} equality for {value}");
            assert_eq!(key, key.clone(), "{family} clone for {value}");
            let copied = key;
            assert_eq!(key, copied, "{family} copy for {value}");
            assert!(!format!("{key:?}").is_empty(), "{family} debug for {value}");
            assert_eq!(
                hash_input(&key),
                hash_input(&from_u128(value)),
                "{family} hash determinism for {value}"
            );
            keys.push(key);
        }

        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "{family}");
        assert!(
            keys.windows(2)
                .all(|pair| hash_input(&pair[0]) != hash_input(&pair[1])),
            "{family} different payload hash input"
        );
    }

    #[test]
    fn direct_key_families_cover_the_complete_representation_boundaries() {
        verify_direct_family("NetworkKey", NetworkKey::from_u128, NetworkKey::as_u128);
        verify_direct_family("NodeKey", NodeKey::from_u128, NodeKey::as_u128);
        verify_direct_family(
            "ConnectionKey",
            ConnectionKey::from_u128,
            ConnectionKey::as_u128,
        );
        verify_direct_family(
            "ModuleInstanceKey",
            ModuleInstanceKey::from_u128,
            ModuleInstanceKey::as_u128,
        );

        macro_rules! verify_typed {
            ($key:ident) => {
                verify_direct_family(
                    concat!(stringify!($key), "<Level>"),
                    $key::<Level>::from_u128,
                    $key::<Level>::as_u128,
                );
                verify_direct_family(
                    concat!(stringify!($key), "<Pulse>"),
                    $key::<Pulse>::from_u128,
                    $key::<Pulse>::as_u128,
                );
            };
        }

        verify_typed!(ModuleInputKey);
        verify_typed!(ModuleOutputKey);
        verify_typed!(InPortKey);
        verify_typed!(OutPortKey);
        verify_typed!(ExternalInputKey);
        verify_typed!(ExternalOutputKey);
    }

    #[test]
    fn direct_keys_have_one_u128_payload_and_required_value_traits() {
        assert_eq!(size_of::<NetworkKey>(), size_of::<u128>());
        assert_eq!(size_of::<NodeKey>(), size_of::<u128>());
        assert_eq!(size_of::<ConnectionKey>(), size_of::<u128>());
        assert_eq!(size_of::<ModuleInstanceKey>(), size_of::<u128>());

        macro_rules! verify_typed {
            ($key:ident) => {
                assert_eq!(size_of::<$key<Level>>(), size_of::<u128>());
                assert_eq!(size_of::<$key<Pulse>>(), size_of::<u128>());
                assert_value_traits::<$key<Level>>();
                assert_value_traits::<$key<Pulse>>();
            };
        }

        verify_typed!(ModuleInputKey);
        verify_typed!(ModuleOutputKey);
        verify_typed!(InPortKey);
        verify_typed!(OutPortKey);
        verify_typed!(ExternalInputKey);
        verify_typed!(ExternalOutputKey);
        assert_value_traits::<SignalSourceKey<Level>>();
        assert_value_traits::<SignalSourceKey<Pulse>>();
        assert_value_traits::<AnyModuleInputKey>();
        assert_value_traits::<AnyModuleOutputKey>();
        assert_value_traits::<AnyInPortKey>();
        assert_value_traits::<AnyOutPortKey>();
        assert_value_traits::<AnyExternalInputKey>();
        assert_value_traits::<AnyExternalOutputKey>();
        assert_value_traits::<AnySignalSourceKey>();
    }

    #[test]
    fn generic_value_traits_do_not_require_marker_traits() {
        assert_value_traits::<ModuleInputKey<TraitHostileSignal>>();
        assert_value_traits::<ModuleOutputKey<TraitHostileSignal>>();
        assert_value_traits::<InPortKey<TraitHostileSignal>>();
        assert_value_traits::<OutPortKey<TraitHostileSignal>>();
        assert_value_traits::<ExternalInputKey<TraitHostileSignal>>();
        assert_value_traits::<ExternalOutputKey<TraitHostileSignal>>();
        assert_value_traits::<SignalSourceKey<TraitHostileSignal>>();
    }

    #[test]
    fn explicit_construction_and_allocator_creation_work_in_constants() {
        assert_eq!(CONST_NODE.as_u128(), 7);
        assert_eq!(CONST_IN_PORT.as_u128(), 9);
        assert_eq!(CONST_ALLOCATOR.namespace, 11);
    }

    #[test]
    fn equal_payload_hashes_are_domain_separated() {
        let value = 77;
        let direct_inputs = [
            hash_input(&NetworkKey::from_u128(value)),
            hash_input(&NodeKey::from_u128(value)),
            hash_input(&ConnectionKey::from_u128(value)),
            hash_input(&ModuleInstanceKey::from_u128(value)),
            hash_input(&ModuleInputKey::<Level>::from_u128(value)),
            hash_input(&ModuleInputKey::<Pulse>::from_u128(value)),
            hash_input(&ModuleOutputKey::<Level>::from_u128(value)),
            hash_input(&ModuleOutputKey::<Pulse>::from_u128(value)),
            hash_input(&InPortKey::<Level>::from_u128(value)),
            hash_input(&InPortKey::<Pulse>::from_u128(value)),
            hash_input(&OutPortKey::<Level>::from_u128(value)),
            hash_input(&OutPortKey::<Pulse>::from_u128(value)),
            hash_input(&ExternalInputKey::<Level>::from_u128(value)),
            hash_input(&ExternalInputKey::<Pulse>::from_u128(value)),
            hash_input(&ExternalOutputKey::<Level>::from_u128(value)),
            hash_input(&ExternalOutputKey::<Pulse>::from_u128(value)),
        ];
        let unique: HashSet<_> = direct_inputs.iter().cloned().collect();
        assert_eq!(unique.len(), direct_inputs.len());

        let external = SignalSourceKey::<Level>::ExternalInput(ExternalInputKey::from_u128(value));
        let out_port = SignalSourceKey::<Level>::OutPort(OutPortKey::from_u128(value));
        assert_ne!(hash_input(&external), hash_input(&out_port));

        let erased_inputs = [
            hash_input(&AnyModuleInputKey::from(
                ModuleInputKey::<Level>::from_u128(value),
            )),
            hash_input(&AnyModuleOutputKey::from(
                ModuleOutputKey::<Level>::from_u128(value),
            )),
            hash_input(&AnyInPortKey::from(InPortKey::<Level>::from_u128(value))),
            hash_input(&AnyOutPortKey::from(OutPortKey::<Level>::from_u128(value))),
            hash_input(&AnyExternalInputKey::from(
                ExternalInputKey::<Level>::from_u128(value),
            )),
            hash_input(&AnyExternalOutputKey::from(
                ExternalOutputKey::<Level>::from_u128(value),
            )),
            hash_input(&AnySignalSourceKey::from(external)),
        ];
        let unique: HashSet<_> = erased_inputs.iter().cloned().collect();
        assert_eq!(unique.len(), erased_inputs.len());

        assert_ne!(
            hash_input(&AnyInPortKey::Level(InPortKey::from_u128(value))),
            hash_input(&AnyInPortKey::Pulse(InPortKey::from_u128(value)))
        );
        assert_ne!(
            hash_input(&AnySignalSourceKey::from(external)),
            hash_input(&AnySignalSourceKey::from(out_port))
        );
    }

    #[test]
    fn equal_payload_categories_and_source_variants_remain_distinct() {
        let value = 5;
        assert_eq!(NodeKey::from_u128(value).as_u128(), value);
        assert_eq!(ConnectionKey::from_u128(value).as_u128(), value);
        assert_eq!(ModuleInstanceKey::from_u128(value).as_u128(), value);
        assert_eq!(InPortKey::<Level>::from_u128(value).as_u128(), value);
        assert_eq!(OutPortKey::<Level>::from_u128(value).as_u128(), value);
        assert_eq!(InPortKey::<Pulse>::from_u128(value).as_u128(), value);
        assert_eq!(
            ExternalInputKey::<Level>::from_u128(value).as_u128(),
            ExternalOutputKey::<Level>::from_u128(value).as_u128()
        );
        assert_eq!(
            ModuleInputKey::<Pulse>::from_u128(value).as_u128(),
            ModuleOutputKey::<Pulse>::from_u128(value).as_u128()
        );

        let external = SignalSourceKey::<Level>::ExternalInput(ExternalInputKey::from_u128(value));
        let out_port = SignalSourceKey::<Level>::OutPort(OutPortKey::from_u128(value));
        assert_ne!(external, out_port);
        assert!(external < out_port);
    }

    #[test]
    fn typed_signal_source_ordering_is_category_then_payload_for_each_kind() {
        macro_rules! verify_source_ordering {
            ($kind:ty) => {{
                let external_low =
                    SignalSourceKey::<$kind>::ExternalInput(ExternalInputKey::from_u128(0));
                let external_high =
                    SignalSourceKey::<$kind>::ExternalInput(ExternalInputKey::from_u128(u128::MAX));
                let out_low = SignalSourceKey::<$kind>::OutPort(OutPortKey::from_u128(0));
                let out_high = SignalSourceKey::<$kind>::OutPort(OutPortKey::from_u128(u128::MAX));

                assert!(external_low < external_high);
                assert!(external_high < out_low);
                assert!(out_low < out_high);
                let reconstructed =
                    SignalSourceKey::<$kind>::ExternalInput(ExternalInputKey::from_u128(0));
                assert_eq!(hash_input(&external_low), hash_input(&reconstructed));
            }};
        }

        verify_source_ordering!(Level);
        verify_source_ordering!(Pulse);
    }

    macro_rules! verify_erased_ordering {
        ($any:ident, $typed:ident) => {{
            let level_low = $any::Level($typed::<Level>::from_u128(0));
            let level_high = $any::Level($typed::<Level>::from_u128(u128::MAX));
            let pulse_low = $any::Pulse($typed::<Pulse>::from_u128(0));
            let pulse_high = $any::Pulse($typed::<Pulse>::from_u128(u128::MAX));
            assert!(level_low < level_high, stringify!($any));
            assert!(level_high < pulse_low, stringify!($any));
            assert!(pulse_low < pulse_high, stringify!($any));
        }};
    }

    #[test]
    fn every_erased_key_uses_explicit_kind_then_payload_ordering() {
        verify_erased_ordering!(AnyModuleInputKey, ModuleInputKey);
        verify_erased_ordering!(AnyModuleOutputKey, ModuleOutputKey);
        verify_erased_ordering!(AnyInPortKey, InPortKey);
        verify_erased_ordering!(AnyOutPortKey, OutPortKey);
        verify_erased_ordering!(AnyExternalInputKey, ExternalInputKey);
        verify_erased_ordering!(AnyExternalOutputKey, ExternalOutputKey);
    }

    #[test]
    fn erased_signal_source_ordering_is_kind_then_category_then_payload() {
        let level_external_low = AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(
            ExternalInputKey::from_u128(0),
        ));
        let level_external_high = AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(
            ExternalInputKey::from_u128(u128::MAX),
        ));
        let level_out_low =
            AnySignalSourceKey::Level(SignalSourceKey::OutPort(OutPortKey::from_u128(0)));
        let level_out_high =
            AnySignalSourceKey::Level(SignalSourceKey::OutPort(OutPortKey::from_u128(u128::MAX)));
        let pulse_external_low = AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(
            ExternalInputKey::from_u128(0),
        ));
        let pulse_external_high = AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(
            ExternalInputKey::from_u128(u128::MAX),
        ));
        let pulse_out_low =
            AnySignalSourceKey::Pulse(SignalSourceKey::OutPort(OutPortKey::from_u128(0)));
        let pulse_out_high =
            AnySignalSourceKey::Pulse(SignalSourceKey::OutPort(OutPortKey::from_u128(u128::MAX)));

        assert!(level_external_low < level_external_high);
        assert!(level_external_high < level_out_low);
        assert!(level_out_low < level_out_high);
        assert!(level_out_high < pulse_external_low);
        assert!(pulse_external_low < pulse_external_high);
        assert!(pulse_external_high < pulse_out_low);
        assert!(pulse_out_low < pulse_out_high);
    }

    macro_rules! verify_key_conversions {
        ($any:ident, $typed:ident) => {{
            for value in [0, 1, u128::MAX] {
                let level = $typed::<Level>::from_u128(value);
                let erased_level: $any = level.into();
                assert_eq!(erased_level.kind(), SignalKind::Level);
                match erased_level {
                    $any::Level(key) => assert_eq!(key.as_u128(), value),
                    $any::Pulse(_) => panic!("level key erased as pulse"),
                }
                assert_eq!($typed::<Level>::try_from(erased_level), Ok(level));
                let error = $typed::<Pulse>::try_from(erased_level)
                    .expect_err("level-to-pulse conversion must fail");
                assert_eq!(error.expected(), SignalKind::Pulse);
                assert_eq!(error.actual(), SignalKind::Level);

                let pulse = $typed::<Pulse>::from_u128(value);
                let erased_pulse: $any = pulse.into();
                assert_eq!(erased_pulse.kind(), SignalKind::Pulse);
                match erased_pulse {
                    $any::Pulse(key) => assert_eq!(key.as_u128(), value),
                    $any::Level(_) => panic!("pulse key erased as level"),
                }
                assert_eq!($typed::<Pulse>::try_from(erased_pulse), Ok(pulse));
                let error = $typed::<Level>::try_from(erased_pulse)
                    .expect_err("pulse-to-level conversion must fail");
                assert_eq!(error.expected(), SignalKind::Level);
                assert_eq!(error.actual(), SignalKind::Pulse);
            }
        }};
    }

    #[test]
    fn every_typed_and_erased_key_conversion_is_lossless_and_checked() {
        verify_key_conversions!(AnyModuleInputKey, ModuleInputKey);
        verify_key_conversions!(AnyModuleOutputKey, ModuleOutputKey);
        verify_key_conversions!(AnyInPortKey, InPortKey);
        verify_key_conversions!(AnyOutPortKey, OutPortKey);
        verify_key_conversions!(AnyExternalInputKey, ExternalInputKey);
        verify_key_conversions!(AnyExternalOutputKey, ExternalOutputKey);
    }

    #[test]
    fn signal_source_conversions_preserve_kind_category_and_payload() {
        for value in [0, 1, u128::MAX] {
            let level_sources = [
                SignalSourceKey::<Level>::from(ExternalInputKey::from_u128(value)),
                SignalSourceKey::<Level>::from(OutPortKey::from_u128(value)),
            ];
            for source in level_sources {
                let erased = AnySignalSourceKey::from(source);
                assert_eq!(erased.kind(), SignalKind::Level);
                assert_eq!(SignalSourceKey::<Level>::try_from(erased), Ok(source));
                let error = SignalSourceKey::<Pulse>::try_from(erased)
                    .expect_err("level-to-pulse source conversion must fail");
                assert_eq!(error.expected(), SignalKind::Pulse);
                assert_eq!(error.actual(), SignalKind::Level);
                match (source, erased) {
                    (
                        SignalSourceKey::ExternalInput(original),
                        AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(converted)),
                    ) => assert_eq!(original, converted),
                    (
                        SignalSourceKey::OutPort(original),
                        AnySignalSourceKey::Level(SignalSourceKey::OutPort(converted)),
                    ) => assert_eq!(original, converted),
                    _ => panic!("level source category changed during erasure"),
                }
            }

            let pulse_sources = [
                SignalSourceKey::<Pulse>::from(ExternalInputKey::from_u128(value)),
                SignalSourceKey::<Pulse>::from(OutPortKey::from_u128(value)),
            ];
            for source in pulse_sources {
                let erased = AnySignalSourceKey::from(source);
                assert_eq!(erased.kind(), SignalKind::Pulse);
                assert_eq!(SignalSourceKey::<Pulse>::try_from(erased), Ok(source));
                let error = SignalSourceKey::<Level>::try_from(erased)
                    .expect_err("pulse-to-level source conversion must fail");
                assert_eq!(error.expected(), SignalKind::Level);
                assert_eq!(error.actual(), SignalKind::Pulse);
                match (source, erased) {
                    (
                        SignalSourceKey::ExternalInput(original),
                        AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(converted)),
                    ) => assert_eq!(original, converted),
                    (
                        SignalSourceKey::OutPort(original),
                        AnySignalSourceKey::Pulse(SignalSourceKey::OutPort(converted)),
                    ) => assert_eq!(original, converted),
                    _ => panic!("pulse source category changed during erasure"),
                }
            }
        }
    }

    #[test]
    fn kind_mismatch_is_structured_and_displayable() {
        fn assert_standard_error<T: std::error::Error>() {}

        assert_standard_error::<KeyKindMismatch>();
        let error = InPortKey::<Level>::try_from(AnyInPortKey::Pulse(InPortKey::from_u128(3)))
            .expect_err("wrong-kind conversion must fail");
        assert_eq!(error.expected(), SignalKind::Level);
        assert_eq!(error.actual(), SignalKind::Pulse);
        assert!(!error.to_string().is_empty());
        let copied = error;
        assert_eq!(hash_input(&error), hash_input(&copied));
    }

    fn expected_payload(namespace: u64, counter: u64) -> u128 {
        (u128::from(namespace) << 64) | u128::from(counter)
    }

    #[test]
    fn all_sixteen_allocator_domains_start_at_zero_and_advance_independently() {
        let namespace = 10;
        let mut allocator = KeyAllocator::new(namespace);

        macro_rules! assert_sequence {
            ($method:ident $(::<$kind:ty>)?) => {{
                assert_eq!(
                    allocator.$method$(::<$kind>)?().as_u128(),
                    expected_payload(namespace, 0),
                    concat!(stringify!($method), " first")
                );
                assert_eq!(
                    allocator.$method$(::<$kind>)?().as_u128(),
                    expected_payload(namespace, 1),
                    concat!(stringify!($method), " second")
                );
                assert_eq!(
                    allocator.$method$(::<$kind>)?().as_u128(),
                    expected_payload(namespace, 2),
                    concat!(stringify!($method), " third")
                );
            }};
        }

        assert_sequence!(network);
        assert_sequence!(node);
        assert_sequence!(connection);
        assert_sequence!(module_instance);
        assert_sequence!(module_input::<Level>);
        assert_sequence!(module_input::<Pulse>);
        assert_sequence!(module_output::<Level>);
        assert_sequence!(module_output::<Pulse>);
        assert_sequence!(in_port::<Level>);
        assert_sequence!(in_port::<Pulse>);
        assert_sequence!(out_port::<Level>);
        assert_sequence!(out_port::<Pulse>);
        assert_sequence!(external_input::<Level>);
        assert_sequence!(external_input::<Pulse>);
        assert_sequence!(external_output::<Level>);
        assert_sequence!(external_output::<Pulse>);
    }

    #[test]
    fn allocator_uses_the_complete_namespace_domain() {
        for namespace in [0, 1, u64::MAX] {
            let mut allocator = KeyAllocator::new(namespace);
            let first = allocator.node().as_u128();
            let second = allocator.node().as_u128();
            assert_eq!(first, expected_payload(namespace, 0));
            assert_eq!(second, expected_payload(namespace, 1));
            assert_eq!(first >> 64, u128::from(namespace));
            assert_eq!(first as u64, 0);
            assert_eq!(second as u64, 1);
        }

        let first_by_namespace: HashSet<_> = [0, 1, u64::MAX]
            .map(|namespace| KeyAllocator::new(namespace).node().as_u128())
            .into_iter()
            .collect();
        assert_eq!(first_by_namespace.len(), 3);
    }

    #[test]
    fn interleaving_unrelated_domains_does_not_perturb_sequences() {
        let namespace = 4;
        let mut allocator = KeyAllocator::new(namespace);

        let node_0 = allocator.node();
        let connection_0 = allocator.connection();
        let level_input_0 = allocator.in_port::<Level>();
        let pulse_input_0 = allocator.in_port::<Pulse>();
        let level_output_0 = allocator.out_port::<Level>();
        let external_input_0 = allocator.external_input::<Level>();
        let external_output_0 = allocator.external_output::<Level>();
        let module_input_0 = allocator.module_input::<Pulse>();
        let node_1 = allocator.node();
        let connection_1 = allocator.connection();
        let level_input_1 = allocator.in_port::<Level>();
        let pulse_input_1 = allocator.in_port::<Pulse>();
        let level_output_1 = allocator.out_port::<Level>();
        let external_input_1 = allocator.external_input::<Level>();
        let external_output_1 = allocator.external_output::<Level>();
        let module_input_1 = allocator.module_input::<Pulse>();

        for payload in [
            node_0.as_u128(),
            connection_0.as_u128(),
            level_input_0.as_u128(),
            pulse_input_0.as_u128(),
            level_output_0.as_u128(),
            external_input_0.as_u128(),
            external_output_0.as_u128(),
            module_input_0.as_u128(),
        ] {
            assert_eq!(payload, expected_payload(namespace, 0));
        }
        assert_eq!(node_1.as_u128(), expected_payload(namespace, 1));
        assert_eq!(connection_1.as_u128(), expected_payload(namespace, 1));
        assert_eq!(level_input_1.as_u128(), expected_payload(namespace, 1));
        assert_eq!(pulse_input_1.as_u128(), expected_payload(namespace, 1));
        assert_eq!(level_output_1.as_u128(), expected_payload(namespace, 1));
        assert_eq!(external_input_1.as_u128(), expected_payload(namespace, 1));
        assert_eq!(external_output_1.as_u128(), expected_payload(namespace, 1));
        assert_eq!(module_input_1.as_u128(), expected_payload(namespace, 1));
        assert!(!format!("{allocator:?}").is_empty());
    }

    #[test]
    fn equal_allocator_histories_reconstruct_equal_keys() {
        let mut left = KeyAllocator::new(8);
        let mut right = KeyAllocator::new(8);

        assert_eq!(left.network(), right.network());
        assert_eq!(left.node(), right.node());
        assert_eq!(left.connection(), right.connection());
        assert_eq!(left.module_instance(), right.module_instance());
        assert_eq!(left.module_input::<Level>(), right.module_input::<Level>());
        assert_eq!(left.module_input::<Pulse>(), right.module_input::<Pulse>());
        assert_eq!(
            left.module_output::<Level>(),
            right.module_output::<Level>()
        );
        assert_eq!(
            left.module_output::<Pulse>(),
            right.module_output::<Pulse>()
        );
        assert_eq!(left.in_port::<Level>(), right.in_port::<Level>());
        assert_eq!(left.in_port::<Pulse>(), right.in_port::<Pulse>());
        assert_eq!(left.out_port::<Level>(), right.out_port::<Level>());
        assert_eq!(left.out_port::<Pulse>(), right.out_port::<Pulse>());
        assert_eq!(
            left.external_input::<Level>(),
            right.external_input::<Level>()
        );
        assert_eq!(
            left.external_input::<Pulse>(),
            right.external_input::<Pulse>()
        );
        assert_eq!(
            left.external_output::<Level>(),
            right.external_output::<Level>()
        );
        assert_eq!(
            left.external_output::<Pulse>(),
            right.external_output::<Pulse>()
        );

        let _ = left.node();
        assert_ne!(left.node(), right.node());
        let mut other_namespace = KeyAllocator::new(9);
        assert_ne!(KeyAllocator::new(8).node(), other_namespace.node());
    }

    #[test]
    fn maximum_counter_is_issued_once_then_only_its_domain_exhausts() {
        let namespace = 12;
        let mut allocator = KeyAllocator::new(namespace);
        allocator.node = Counter {
            next: u64::MAX,
            exhausted: false,
        };

        assert_eq!(
            allocator.node().as_u128(),
            expected_payload(namespace, u64::MAX)
        );
        assert!(allocator.node.exhausted);
        assert_eq!(allocator.node.next, u64::MAX);

        let repeated = catch_unwind(AssertUnwindSafe(|| allocator.node()));
        assert!(repeated.is_err());
        assert!(allocator.node.exhausted);
        assert_eq!(allocator.node.next, u64::MAX);
        assert_eq!(
            allocator.connection().as_u128(),
            expected_payload(namespace, 0)
        );
    }
}
