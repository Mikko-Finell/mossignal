//! Mutable machine lifecycle over an immutable compiled topology.

use crate::compile::CompiledNetwork;
use crate::identity::NetworkFingerprint;
use crate::key::{ExternalInputKey, ExternalOutputKey};
use crate::policy::{RuntimePolicy, RuntimePolicyId};
use crate::signal::{Level, LogicLevel};
use crate::time::Time;
use crate::transaction::{CauseRef, ProvenanceView};
use core::fmt;
use std::collections::BTreeMap;

/// The opaque machine-local revision of the currently installed topology.
///
/// A revision is distinct from the semantic network fingerprint. Its initial
/// numeric representation is private and is not a persistence-format promise.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NetworkRevision(u64);

impl NetworkRevision {
    const INITIAL: Self = Self(0);

    #[cfg(test)]
    pub(crate) const fn from_value(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for NetworkRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "NetworkRevision({self})")
    }
}

impl fmt::Display for NetworkRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

/// The runtime lifecycle state of a [`Machine`].
#[non_exhaustive]
pub enum MachineStatus<D> {
    /// The machine has not yet received a complete initializing input snapshot.
    AwaitingInitialization,
    /// The machine has committed initialization and has a current logical time.
    Ready { now: Time<D> },
}

impl<D> Clone for MachineStatus<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for MachineStatus<D> {}

impl<D> PartialEq for MachineStatus<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AwaitingInitialization, Self::AwaitingInitialization) => true,
            (Self::Ready { now: left }, Self::Ready { now: right }) => left == right,
            _ => false,
        }
    }
}

impl<D> Eq for MachineStatus<D> {}

impl<D> fmt::Debug for MachineStatus<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AwaitingInitialization => formatter.write_str("AwaitingInitialization"),
            Self::Ready { now } => formatter.debug_struct("Ready").field("now", now).finish(),
        }
    }
}

pub(crate) struct MachineStore<D> {
    pub(crate) status: MachineStatus<D>,
    pub(crate) revision: NetworkRevision,
    pub(crate) external_levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pub(crate) settled_levels: Vec<LogicLevel>,
    pub(crate) output_baselines: BTreeMap<ExternalOutputKey<Level>, LogicLevel>,
    pub(crate) input_causes: BTreeMap<ExternalInputKey<Level>, CauseRef>,
    pub(crate) output_causes: BTreeMap<ExternalOutputKey<Level>, CauseRef>,
    pub(crate) provenance: Option<ProvenanceView<D>>,
}

impl<D> Clone for MachineStore<D> {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            revision: self.revision,
            external_levels: self.external_levels.clone(),
            settled_levels: self.settled_levels.clone(),
            output_baselines: self.output_baselines.clone(),
            input_causes: self.input_causes.clone(),
            output_causes: self.output_causes.clone(),
            provenance: self.provenance.clone(),
        }
    }
}

/// One mutable semantic execution instance of a compiled network.
///
/// The canonical type carries lifecycle at runtime, so both uninitialized and
/// ready machines have the same `Machine<D>` type. A newly spawned machine has
/// no current time or settled runtime values.
///
/// `Machine` deliberately does not provide ordinary cloning:
///
/// ```compile_fail
/// use mossignal::Machine;
///
/// fn clone_machine<D>(machine: &Machine<D>) -> Machine<D> {
///     machine.clone()
/// }
/// ```
pub struct Machine<D> {
    pub(crate) compiled: CompiledNetwork<D>,
    pub(crate) policy: RuntimePolicy,
    // SPEC: docs/specs/processor_and_runtime_architecture.md §6 "Machine lifecycle states"
    // Absence before initialization is lifecycle state, never fabricated Low values.
    pub(crate) store: MachineStore<D>,
}

impl<D> Machine<D> {
    pub(crate) fn new(compiled: CompiledNetwork<D>, policy: RuntimePolicy) -> Self {
        Self {
            compiled,
            policy,
            store: MachineStore {
                status: MachineStatus::AwaitingInitialization,
                revision: NetworkRevision::INITIAL,
                external_levels: BTreeMap::new(),
                settled_levels: Vec::new(),
                output_baselines: BTreeMap::new(),
                input_causes: BTreeMap::new(),
                output_causes: BTreeMap::new(),
                provenance: None,
            },
        }
    }

    /// Returns the current runtime lifecycle state.
    #[must_use]
    pub fn status(&self) -> MachineStatus<D> {
        self.store.status
    }

    /// Returns whether the machine has committed a successful initialization.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        matches!(self.store.status, MachineStatus::Ready { .. })
    }

    /// Returns the current logical time, or `None` before initialization.
    #[must_use]
    pub fn now(&self) -> Option<Time<D>> {
        match self.store.status {
            MachineStatus::AwaitingInitialization => None,
            MachineStatus::Ready { now } => Some(now),
        }
    }

    /// Returns the machine-local revision of the installed topology.
    #[must_use]
    pub fn revision(&self) -> NetworkRevision {
        self.store.revision
    }

    /// Returns the semantic fingerprint of the installed compiled topology.
    #[must_use]
    pub fn fingerprint(&self) -> NetworkFingerprint {
        self.compiled.fingerprint()
    }

    /// Returns the immutable compiled topology installed in this machine.
    #[must_use]
    pub fn compiled(&self) -> &CompiledNetwork<D> {
        &self.compiled
    }

    /// Returns the exact validated runtime policy associated at spawning.
    #[must_use]
    pub fn runtime_policy(&self) -> &RuntimePolicy {
        &self.policy
    }

    /// Returns the semantic identity of the associated runtime policy.
    #[must_use]
    pub fn runtime_policy_id(&self) -> RuntimePolicyId {
        self.policy.id()
    }

    /// Returns one authoritative external level after initialization.
    #[must_use]
    pub fn external_level(&self, input: ExternalInputKey<Level>) -> Option<LogicLevel> {
        if !self.is_initialized() {
            return None;
        }
        self.store.external_levels.get(&input).copied()
    }

    /// Returns one established external level-output baseline after initialization.
    #[must_use]
    pub fn output_level(&self, output: ExternalOutputKey<Level>) -> Option<LogicLevel> {
        if !self.is_initialized() {
            return None;
        }
        self.store.output_baselines.get(&output).copied()
    }

    /// Returns the current retained cause of an established external level output.
    #[must_use]
    pub fn output_cause(&self, output: ExternalOutputKey<Level>) -> Option<CauseRef> {
        if !self.is_initialized() {
            return None;
        }
        self.store.output_causes.get(&output).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeDomainId;
    use crate::authored::{NodeDef, NodeKind, NodePorts, UncheckedNetwork};
    use crate::key::{NetworkKey, NodeKey, OutPortKey};
    use crate::metadata::DiagnosticMeta;
    use crate::signal::{Level, LogicLevel};
    use std::collections::BTreeMap;

    #[derive(PartialEq)]
    struct Ticks;

    fn compiled() -> CompiledNetwork<Ticks> {
        let network = UncheckedNetwork::new(
            NetworkKey::from_u128(1),
            TimeDomainId::from_u128(2),
            DiagnosticMeta::default(),
            vec![NodeDef::new(
                NodeKey::from_u128(3),
                NodeKind::constant(LogicLevel::High),
                NodePorts::new(Vec::new(), vec![OutPortKey::<Level>::from_u128(4).into()]),
                DiagnosticMeta::default(),
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        network
            .validate()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must validate"))
            .compile()
            .require_artifact()
            .unwrap_or_else(|_| panic!("fixture must compile"))
    }

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

    #[test]
    fn spawn_retains_topology_and_exact_policy_without_initializing_values() {
        let compiled = compiled();
        let fingerprint = compiled.fingerprint();
        let expected_policy = policy([1, 2, 3, 4, 5]);
        let policy_id = expected_policy.id();

        let machine = compiled.spawn(expected_policy.clone());

        assert_eq!(machine.status(), MachineStatus::AwaitingInitialization);
        assert!(!machine.is_initialized());
        assert_eq!(machine.now(), None);
        assert_eq!(machine.fingerprint(), fingerprint);
        assert_eq!(machine.compiled().fingerprint(), fingerprint);
        assert_eq!(machine.runtime_policy(), &expected_policy);
        assert_eq!(machine.runtime_policy_id(), policy_id);
    }

    #[test]
    fn equivalent_spawns_have_deterministic_initial_revisions() {
        let compiled = compiled();
        let first = compiled.spawn(policy([1, 2, 3, 4, 5]));
        let second = compiled.spawn(policy([1, 2, 3, 4, 5]));

        assert_eq!(first.revision(), second.revision());
        assert_eq!(first.revision(), NetworkRevision::INITIAL);
    }

    #[test]
    fn spawned_machines_own_independent_mutable_lifecycle_stores() {
        let compiled = compiled();
        let mut first = compiled.spawn(policy([1, 2, 3, 4, 5]));
        let second = compiled.spawn(policy([1, 2, 3, 4, 5]));
        let expected_evaluation = compiled.evaluate_full(&BTreeMap::new());

        assert!(!core::ptr::eq(&first.store, &second.store));
        first.store.status = MachineStatus::Ready {
            now: Time::from_ticks(17),
        };
        first.store.revision = NetworkRevision(7);

        assert_eq!(
            first.status(),
            MachineStatus::Ready {
                now: Time::from_ticks(17)
            }
        );
        assert_eq!(first.now(), Some(Time::from_ticks(17)));
        assert_eq!(first.revision(), NetworkRevision(7));
        assert_eq!(second.status(), MachineStatus::AwaitingInitialization);
        assert_eq!(second.now(), None);
        assert_eq!(second.revision(), NetworkRevision::INITIAL);
        assert_eq!(
            first.compiled().network_key(),
            second.compiled().network_key()
        );
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.runtime_policy_id(), second.runtime_policy_id());
        assert_eq!(
            first.compiled().evaluate_full(&BTreeMap::new()),
            expected_evaluation
        );
        assert_eq!(
            second.compiled().evaluate_full(&BTreeMap::new()),
            expected_evaluation
        );
    }
}
