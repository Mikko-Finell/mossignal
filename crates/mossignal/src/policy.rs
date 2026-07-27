//! Immutable runtime-policy construction and semantic identity.

use crate::identity::Cbor;
use core::fmt;

const RUNTIME_POLICY_DOMAIN: &str = "mossignal/runtime_policy_id/v1";

/// One named limit in the initial runtime-policy schema.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuntimePolicyLimit {
    MaxInternalReactions,
    MaxEvaluatedOperations,
    MaxPendingEvents,
    MaxEventsCreatedPerTransaction,
    MaxRequiredProvenanceGrowth,
}

impl RuntimePolicyLimit {
    const fn parameter_key(self) -> &'static str {
        match self {
            Self::MaxInternalReactions => "max_internal_reactions",
            Self::MaxEvaluatedOperations => "max_evaluated_operations",
            Self::MaxPendingEvents => "max_pending_events",
            Self::MaxEventsCreatedPerTransaction => "max_events_created_per_transaction",
            Self::MaxRequiredProvenanceGrowth => "max_required_provenance_growth",
        }
    }
}

impl fmt::Display for RuntimePolicyLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.parameter_key())
    }
}

/// A failure while constructing a validated runtime policy.
///
/// The initial schema accepts the complete `u64` domain, so
/// [`PolicyFailure::InvalidLimit`] is reserved for a future declared numeric
/// restriction and is not currently returned by [`RuntimePolicyBuilder::build`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyFailure {
    /// A required semantic limit was not supplied.
    #[non_exhaustive]
    MissingLimit { limit: RuntimePolicyLimit },
    /// A supplied limit violated its declared numeric domain.
    #[non_exhaustive]
    InvalidLimit {
        limit: RuntimePolicyLimit,
        value: u64,
    },
}

impl PolicyFailure {
    /// Returns the catalogue code represented by this construction failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingLimit { .. } => "runtime.policy_missing_limit",
            Self::InvalidLimit { .. } => "runtime.policy_invalid_limit",
        }
    }

    /// Returns the policy parameter involved in this failure.
    #[must_use]
    pub const fn limit(self) -> RuntimePolicyLimit {
        match self {
            Self::MissingLimit { limit } | Self::InvalidLimit { limit, .. } => limit,
        }
    }

    /// Returns the invalid supplied value, or `None` for an absent limit.
    #[must_use]
    pub const fn invalid_value(self) -> Option<u64> {
        match self {
            Self::MissingLimit { .. } => None,
            Self::InvalidLimit { value, .. } => Some(value),
        }
    }
}

impl fmt::Display for PolicyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLimit { limit } => write!(formatter, "runtime policy omits {limit}"),
            Self::InvalidLimit { limit, value } => {
                write!(
                    formatter,
                    "runtime policy value {value} is invalid for {limit}"
                )
            }
        }
    }
}

impl std::error::Error for PolicyFailure {}

/// The opaque canonical identity of one semantic runtime policy.
///
/// Runtime policy identity is distinct from network identity:
///
/// ```compile_fail
/// use mossignal::{NetworkFingerprint, RuntimePolicyId};
/// fn accepts_policy_id(_: RuntimePolicyId) {}
/// fn wrong(value: NetworkFingerprint) { accepts_policy_id(value); }
/// ```
///
/// It is also distinct from input-schema identity:
///
/// ```compile_fail
/// use mossignal::{InputSchemaFingerprint, RuntimePolicyId};
/// fn accepts_policy_id(_: RuntimePolicyId) {}
/// fn wrong(value: InputSchemaFingerprint) { accepts_policy_id(value); }
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuntimePolicyId([u8; 32]);

impl RuntimePolicyId {
    /// Returns the fixed canonical digest bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    const fn from_digest(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for RuntimePolicyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "RuntimePolicyId({self})")
    }
}

impl fmt::Display for RuntimePolicyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PolicyValues {
    max_internal_reactions: u64,
    max_evaluated_operations: u64,
    max_pending_events: u64,
    max_events_created_per_transaction: u64,
    max_required_provenance_growth: u64,
}

/// An immutable validated set of limits that may affect execution success.
///
/// Every initial limit must be supplied explicitly:
///
/// ```
/// use mossignal::RuntimePolicy;
///
/// let policy = RuntimePolicy::builder()
///     .max_internal_reactions(10_000)
///     .max_evaluated_operations(100_000)
///     .max_pending_events(1_000)
///     .max_events_created_per_transaction(1_000)
///     .max_required_provenance_growth(10_000)
///     .build()?;
/// # Ok::<(), mossignal::PolicyFailure>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePolicy {
    values: PolicyValues,
    id: RuntimePolicyId,
}

impl RuntimePolicy {
    /// Starts an empty builder that requires every initial semantic limit.
    #[must_use]
    pub const fn builder() -> RuntimePolicyBuilder {
        RuntimePolicyBuilder::new()
    }

    /// Returns this exact policy's canonical semantic identity.
    #[must_use]
    pub const fn id(&self) -> RuntimePolicyId {
        self.id
    }

    /// Returns the maximum number of internal reactions.
    #[must_use]
    pub const fn max_internal_reactions(&self) -> u64 {
        self.values.max_internal_reactions
    }

    /// Returns the maximum number of evaluated operations.
    #[must_use]
    pub const fn max_evaluated_operations(&self) -> u64 {
        self.values.max_evaluated_operations
    }

    /// Returns the maximum number of pending events.
    #[must_use]
    pub const fn max_pending_events(&self) -> u64 {
        self.values.max_pending_events
    }

    /// Returns the maximum number of events created by one transaction.
    #[must_use]
    pub const fn max_events_created_per_transaction(&self) -> u64 {
        self.values.max_events_created_per_transaction
    }

    /// Returns the maximum required provenance growth.
    #[must_use]
    pub const fn max_required_provenance_growth(&self) -> u64 {
        self.values.max_required_provenance_growth
    }
}

/// An owned builder for a complete immutable [`RuntimePolicy`].
#[derive(Debug)]
pub struct RuntimePolicyBuilder {
    max_internal_reactions: Option<u64>,
    max_evaluated_operations: Option<u64>,
    max_pending_events: Option<u64>,
    max_events_created_per_transaction: Option<u64>,
    max_required_provenance_growth: Option<u64>,
}

impl RuntimePolicyBuilder {
    const fn new() -> Self {
        Self {
            max_internal_reactions: None,
            max_evaluated_operations: None,
            max_pending_events: None,
            max_events_created_per_transaction: None,
            max_required_provenance_growth: None,
        }
    }

    /// Sets the maximum number of internal reactions.
    #[must_use]
    pub const fn max_internal_reactions(mut self, value: u64) -> Self {
        self.max_internal_reactions = Some(value);
        self
    }

    /// Sets the maximum number of evaluated operations.
    #[must_use]
    pub const fn max_evaluated_operations(mut self, value: u64) -> Self {
        self.max_evaluated_operations = Some(value);
        self
    }

    /// Sets the maximum number of pending events.
    #[must_use]
    pub const fn max_pending_events(mut self, value: u64) -> Self {
        self.max_pending_events = Some(value);
        self
    }

    /// Sets the maximum number of events created by one transaction.
    #[must_use]
    pub const fn max_events_created_per_transaction(mut self, value: u64) -> Self {
        self.max_events_created_per_transaction = Some(value);
        self
    }

    /// Sets the maximum required provenance growth.
    #[must_use]
    pub const fn max_required_provenance_growth(mut self, value: u64) -> Self {
        self.max_required_provenance_growth = Some(value);
        self
    }

    /// Validates completeness and builds the immutable policy.
    pub fn build(self) -> Result<RuntimePolicy, PolicyFailure> {
        let values = PolicyValues {
            max_internal_reactions: required(
                self.max_internal_reactions,
                RuntimePolicyLimit::MaxInternalReactions,
            )?,
            max_evaluated_operations: required(
                self.max_evaluated_operations,
                RuntimePolicyLimit::MaxEvaluatedOperations,
            )?,
            max_pending_events: required(
                self.max_pending_events,
                RuntimePolicyLimit::MaxPendingEvents,
            )?,
            max_events_created_per_transaction: required(
                self.max_events_created_per_transaction,
                RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
            )?,
            max_required_provenance_growth: required(
                self.max_required_provenance_growth,
                RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
            )?,
        };
        let bytes = canonical_values(&values);
        let id = RuntimePolicyId::from_digest(*blake3::hash(&bytes).as_bytes());
        Ok(RuntimePolicy { values, id })
    }
}

fn required(value: Option<u64>, limit: RuntimePolicyLimit) -> Result<u64, PolicyFailure> {
    value.ok_or(PolicyFailure::MissingLimit { limit })
}

fn canonical_values(values: &PolicyValues) -> Vec<u8> {
    let mut writer = Cbor::default();
    writer.record_start(3);
    writer.field("domain", |writer| writer.text(RUNTIME_POLICY_DOMAIN));
    writer.field("payload", |writer| {
        writer.record_start(5);
        writer.field("max_evaluated_operations", |writer| {
            writer.uint(values.max_evaluated_operations)
        });
        writer.field("max_events_created_per_transaction", |writer| {
            writer.uint(values.max_events_created_per_transaction)
        });
        writer.field("max_internal_reactions", |writer| {
            writer.uint(values.max_internal_reactions)
        });
        writer.field("max_pending_events", |writer| {
            writer.uint(values.max_pending_events)
        });
        writer.field("max_required_provenance_growth", |writer| {
            writer.uint(values.max_required_provenance_growth)
        });
    });
    writer.field("version", |writer| writer.uint(1));
    writer.finish()
}

#[cfg(test)]
fn canonical_digest_input(policy: &RuntimePolicy) -> Vec<u8> {
    canonical_values(&policy.values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Debug;
    use core::hash::Hash;

    fn policy(values: [u64; 5]) -> RuntimePolicy {
        RuntimePolicy::builder()
            .max_internal_reactions(values[0])
            .max_evaluated_operations(values[1])
            .max_pending_events(values[2])
            .max_events_created_per_transaction(values[3])
            .max_required_provenance_growth(values[4])
            .build()
            .unwrap_or_else(|failure| panic!("complete policy must build: {failure}"))
    }

    fn builder_without(missing: RuntimePolicyLimit) -> RuntimePolicyBuilder {
        let mut builder = RuntimePolicy::builder();
        if missing != RuntimePolicyLimit::MaxInternalReactions {
            builder = builder.max_internal_reactions(1);
        }
        if missing != RuntimePolicyLimit::MaxEvaluatedOperations {
            builder = builder.max_evaluated_operations(2);
        }
        if missing != RuntimePolicyLimit::MaxPendingEvents {
            builder = builder.max_pending_events(3);
        }
        if missing != RuntimePolicyLimit::MaxEventsCreatedPerTransaction {
            builder = builder.max_events_created_per_transaction(4);
        }
        if missing != RuntimePolicyLimit::MaxRequiredProvenanceGrowth {
            builder = builder.max_required_provenance_growth(5);
        }
        builder
    }

    #[test]
    fn complete_policy_is_inspectable_and_accepts_the_full_u64_domain() {
        let zero = policy([0; 5]);
        assert_eq!(zero.max_internal_reactions(), 0);
        assert_eq!(zero.max_evaluated_operations(), 0);
        assert_eq!(zero.max_pending_events(), 0);
        assert_eq!(zero.max_events_created_per_transaction(), 0);
        assert_eq!(zero.max_required_provenance_growth(), 0);

        let maximum = policy([u64::MAX; 5]);
        assert_eq!(maximum.max_internal_reactions(), u64::MAX);
        assert_eq!(maximum.max_evaluated_operations(), u64::MAX);
        assert_eq!(maximum.max_pending_events(), u64::MAX);
        assert_eq!(maximum.max_events_created_per_transaction(), u64::MAX);
        assert_eq!(maximum.max_required_provenance_growth(), u64::MAX);
    }

    #[test]
    fn builder_reports_each_missing_limit_precisely() {
        let limits = [
            RuntimePolicyLimit::MaxInternalReactions,
            RuntimePolicyLimit::MaxEvaluatedOperations,
            RuntimePolicyLimit::MaxPendingEvents,
            RuntimePolicyLimit::MaxEventsCreatedPerTransaction,
            RuntimePolicyLimit::MaxRequiredProvenanceGrowth,
        ];

        for limit in limits {
            assert_eq!(
                builder_without(limit).build(),
                Err(PolicyFailure::MissingLimit { limit })
            );
        }
    }

    #[test]
    fn failure_categories_keep_their_catalogue_codes() {
        let missing = PolicyFailure::MissingLimit {
            limit: RuntimePolicyLimit::MaxPendingEvents,
        };
        let invalid = PolicyFailure::InvalidLimit {
            limit: RuntimePolicyLimit::MaxPendingEvents,
            value: 0,
        };
        assert_eq!(missing.code(), "runtime.policy_missing_limit");
        assert_eq!(invalid.code(), "runtime.policy_invalid_limit");
    }

    #[test]
    fn setter_order_does_not_change_canonical_identity() {
        let forward = policy([1, 2, 3, 4, 5]);
        let reverse = RuntimePolicy::builder()
            .max_required_provenance_growth(5)
            .max_events_created_per_transaction(4)
            .max_pending_events(3)
            .max_evaluated_operations(2)
            .max_internal_reactions(1)
            .build()
            .unwrap_or_else(|failure| panic!("complete policy must build: {failure}"));

        assert_eq!(forward, reverse);
        assert_eq!(forward.id(), reverse.id());
        assert_eq!(
            canonical_digest_input(&forward),
            canonical_digest_input(&reverse)
        );
    }

    #[test]
    fn every_semantic_limit_changes_identity() {
        let base = policy([1, 2, 3, 4, 5]).id();
        for values in [
            [9, 2, 3, 4, 5],
            [1, 9, 3, 4, 5],
            [1, 2, 9, 4, 5],
            [1, 2, 3, 9, 5],
            [1, 2, 3, 4, 9],
        ] {
            assert_ne!(base, policy(values).id());
        }
    }

    #[test]
    fn runtime_policy_id_is_an_ordinary_opaque_value() {
        fn assert_traits<T: Clone + Copy + Eq + Hash + Ord + Debug>() {}
        assert_traits::<RuntimePolicyId>();

        let id = policy([1, 2, 3, 4, 5]).id();
        assert_eq!(id.to_string().len(), 64);
        assert!(id.to_string().bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(format!("{id:?}"), format!("RuntimePolicyId({id})"));
    }

    #[test]
    fn version_one_policy_identity_has_a_golden_vector() {
        assert_eq!(
            policy([1, 2, 3, 4, 5]).id().to_string(),
            "62e410cd29ddd5ddfae4888f648ad290d5c9633f25e7b35c2ff727e4f9ffa04b"
        );
    }
}
