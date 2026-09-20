//! Owned persistent conditions and the candidate episode transition model.
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagnostics::{
    ConflictControls, ConflictEvidence, DiagnosticCode, NodeEvidence, Problem, ProblemDelivery,
    ProblemEvidence, SubjectRef,
};
use crate::key::NetworkKey;
use crate::signal::LogicLevel;
use crate::time::Time;
use crate::{CauseRef, ConflictPolicy, NodeSubject, ProvenanceView, QualifiedNodeRef};

/// Opaque identity of a catalogue condition on one stable owning subject.
// SPEC: docs/specs/contracts/persistent-diagnostic-episodes.yaml "catalogue-safe-condition"
// Qualified owners, rather than a Problem's private flattened node, define key identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiagnosticConditionKey {
    owner: NodeSubject,
    code: DiagnosticCode,
    discriminator: u8,
}

impl DiagnosticConditionKey {
    /// Returns the underlying condition's catalogue code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }
    /// Returns the stable primitive owner, including its full instance path.
    #[must_use]
    pub const fn owner(&self) -> &NodeSubject {
        &self.owner
    }

    fn from_problem<D>(problem: &Problem<D>) -> Option<Self> {
        if !problem
            .code()
            .allows_delivery(ProblemDelivery::PersistentEpisode)
        {
            return None;
        }
        let evidence = conflict_evidence(problem)?;
        if evidence.policy != ConflictPolicy::RetainAndDiagnose
            || evidence.controls
                != (ConflictControls::Level {
                    set: LogicLevel::High,
                    reset: LogicLevel::High,
                })
        {
            return None;
        }
        let owner = match &evidence.node {
            NodeEvidence::Node(node) if problem.primary() == &SubjectRef::Node(*node) => {
                NodeSubject::Node(*node)
            }
            NodeEvidence::Qualified { instances, node } if !instances.is_empty() => {
                let qualified = QualifiedNodeRef::new(instances.clone(), *node);
                if problem.primary() != &SubjectRef::QualifiedNode(qualified.clone()) {
                    return None;
                }
                NodeSubject::Qualified(qualified)
            }
            _ => return None,
        };
        // The initial catalogue has one retained-level-conflict condition per owner.
        // Its discriminator is the singleton 0; time and evidence do not split it.
        Some(Self {
            owner,
            code: problem.code(),
            discriminator: 0,
        })
    }
}

/// Stable, opaque identity of a persistent condition within one network identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticEpisodeId([u8; 32]);

impl DiagnosticEpisodeId {
    /// Returns the deterministic opaque identity bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    fn derive(network: NetworkKey, condition: &DiagnosticConditionKey) -> Self {
        // SPEC: docs/specs/contracts/persistent-diagnostic-episodes.yaml "stable-episode-identity"
        // Network identity plus the full semantic owner excludes flattened slots and cause IDs.
        let mut hash = blake3::Hasher::new();
        hash.update(b"mossignal/diagnostic_episode/v1\0");
        hash.update(&network.as_u128().to_be_bytes());
        hash.update(&(condition.code.as_str().len() as u64).to_be_bytes());
        hash.update(condition.code.as_str().as_bytes());
        match &condition.owner {
            NodeSubject::Node(node) => {
                hash.update(&[0]);
                hash.update(&node.as_u128().to_be_bytes());
            }
            NodeSubject::Qualified(node) => {
                hash.update(&[1]);
                hash.update(&(node.instances().len() as u64).to_be_bytes());
                for instance in node.instances() {
                    hash.update(&instance.as_u128().to_be_bytes());
                }
                hash.update(&node.node().as_u128().to_be_bytes());
            }
        }
        hash.update(&[condition.discriminator]);
        Self(*hash.finalize().as_bytes())
    }
}

/// An owned view of one current condition, retaining its evidence's causal view.
pub struct ActiveDiagnosticEpisode<D> {
    identity: DiagnosticEpisodeId,
    condition: DiagnosticConditionKey,
    current: Arc<Problem<D>>,
    began_at: Time<D>,
    last_material_change: Time<D>,
    cause: CauseRef,
    provenance: ProvenanceView<D>,
}

impl<D: std::fmt::Debug> std::fmt::Debug for ActiveDiagnosticEpisode<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveDiagnosticEpisode")
            .field("identity", &self.identity)
            .field("condition", &self.condition)
            .field("current", &self.current)
            .field("began_at", &self.began_at)
            .field("last_material_change", &self.last_material_change)
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}

impl<D> Clone for ActiveDiagnosticEpisode<D> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity,
            condition: self.condition.clone(),
            current: Arc::clone(&self.current),
            began_at: self.began_at,
            last_material_change: self.last_material_change,
            cause: self.cause,
            provenance: self.provenance.clone(),
        }
    }
}

impl<D> ActiveDiagnosticEpisode<D> {
    /// Returns the network-scoped episode identity.
    #[must_use]
    pub const fn identity(&self) -> DiagnosticEpisodeId {
        self.identity
    }
    /// Returns the underlying condition identity.
    #[must_use]
    pub const fn condition(&self) -> &DiagnosticConditionKey {
        &self.condition
    }
    /// Returns the problem at the most recent material condition change.
    #[must_use]
    pub fn current(&self) -> &Problem<D> {
        &self.current
    }
    /// Returns when this continuous active interval began.
    #[must_use]
    pub const fn began_at(&self) -> Time<D> {
        self.began_at
    }
    /// Returns when the meaningful evidence last changed.
    #[must_use]
    pub const fn last_material_change(&self) -> Time<D> {
        self.last_material_change
    }
    /// Returns the cause of the retained evidence, resolved by this inspection's view.
    #[must_use]
    pub const fn cause(&self) -> CauseRef {
        self.cause
    }
    /// Returns the immutable view that owns the retained evidence cause.
    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceView<D> {
        &self.provenance
    }
}

/// The lifecycle transition of a persistent condition; these are not problem codes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticEpisodeChangeKind {
    /// A condition became active.
    Began,
    /// The same active condition's evidence materially changed.
    Changed,
    /// The condition ceased to be active.
    Resolved,
    /// Owning semantics were terminated (reserved for topology replacement).
    Terminated,
}

/// An owned committed lifecycle change with exact prior and successor problems.
#[derive(Debug)]
pub struct DiagnosticEpisodeChange<D> {
    identity: DiagnosticEpisodeId,
    kind: DiagnosticEpisodeChangeKind,
    at: Time<D>,
    before: Option<Arc<Problem<D>>>,
    after: Option<Arc<Problem<D>>>,
    cause: CauseRef,
}

impl<D> DiagnosticEpisodeChange<D> {
    /// Returns the affected episode identity.
    #[must_use]
    pub const fn identity(&self) -> DiagnosticEpisodeId {
        self.identity
    }
    /// Returns the lifecycle transition kind.
    #[must_use]
    pub const fn kind(&self) -> DiagnosticEpisodeChangeKind {
        self.kind
    }
    /// Returns the actual reaction time of this change.
    #[must_use]
    pub const fn at(&self) -> Time<D> {
        self.at
    }
    /// Returns prior active evidence; absent for Began.
    #[must_use]
    pub fn before(&self) -> Option<&Problem<D>> {
        self.before.as_deref()
    }
    /// Returns successor active evidence; absent for Resolved or Terminated.
    #[must_use]
    pub fn after(&self) -> Option<&Problem<D>> {
        self.after.as_deref()
    }
    /// Returns the change's cause, resolved by the containing transaction result.
    #[must_use]
    pub const fn cause(&self) -> CauseRef {
        self.cause
    }
    pub(crate) fn remap_cause(&mut self, cause: CauseRef) {
        self.cause = cause;
    }
}

pub(crate) type ActiveEpisodes<D> = BTreeMap<DiagnosticConditionKey, ActiveDiagnosticEpisode<D>>;

fn conflict_evidence<D>(problem: &Problem<D>) -> Option<&ConflictEvidence> {
    match problem.evidence() {
        ProblemEvidence::RuntimeLevelLatchConflictRetained { evidence, .. } => Some(evidence),
        _ => None,
    }
}

fn materially_equal<D>(left: &Problem<D>, right: &Problem<D>) -> bool {
    match (conflict_evidence(left), conflict_evidence(right)) {
        (Some(left), Some(right)) => {
            left.node == right.node
                && left.policy == right.policy
                && left.previous == right.previous
                && left.controls == right.controls
                && left.revision == right.revision
        }
        _ => false,
    }
}

/// Reconciles one complete settled reaction against candidate active state.
/// `causes` includes all level latches, so resolution also has current causal support.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reconcile<D>(
    active: &mut ActiveEpisodes<D>,
    changes: &mut Vec<DiagnosticEpisodeChange<D>>,
    network: NetworkKey,
    at: Time<D>,
    problems: Vec<Problem<D>>,
    causes: &BTreeMap<NodeSubject, CauseRef>,
    provenance: &ProvenanceView<D>,
) {
    let mut current = BTreeMap::new();
    for problem in problems {
        let Some(key) = DiagnosticConditionKey::from_problem(&problem) else {
            panic!(
                "settled retained level conflict must form one catalogue-valid episode condition"
            );
        };
        let evidence = conflict_evidence(&problem);
        assert!(
            evidence.is_some_and(|evidence| evidence.at_ticks == at.ticks()),
            "candidate episode evidence must identify its exact reaction time"
        );
        assert!(
            current.insert(key, Arc::new(problem)).is_none(),
            "one settled primitive must produce at most one retained level conflict per reaction"
        );
    }
    let keys = active
        .keys()
        .chain(current.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    for key in keys {
        let Some(cause) = causes.get(&key.owner).copied() else {
            panic!("every active or current episode owner must retain its settled reaction cause");
        };
        match (active.get(&key), current.remove(&key)) {
            (None, Some(problem)) => {
                let identity = DiagnosticEpisodeId::derive(network, &key);
                changes.push(DiagnosticEpisodeChange {
                    identity,
                    kind: DiagnosticEpisodeChangeKind::Began,
                    at,
                    before: None,
                    after: Some(Arc::clone(&problem)),
                    cause,
                });
                active.insert(
                    key.clone(),
                    ActiveDiagnosticEpisode {
                        identity,
                        condition: key,
                        current: problem,
                        began_at: at,
                        last_material_change: at,
                        cause,
                        provenance: provenance.clone(),
                    },
                );
            }
            (Some(previous), Some(problem)) if !materially_equal(&previous.current, &problem) => {
                let identity = previous.identity;
                let began_at = previous.began_at;
                changes.push(DiagnosticEpisodeChange {
                    identity,
                    kind: DiagnosticEpisodeChangeKind::Changed,
                    at,
                    before: Some(Arc::clone(&previous.current)),
                    after: Some(Arc::clone(&problem)),
                    cause,
                });
                active.insert(
                    key.clone(),
                    ActiveDiagnosticEpisode {
                        identity,
                        condition: key,
                        current: problem,
                        began_at,
                        last_material_change: at,
                        cause,
                        provenance: provenance.clone(),
                    },
                );
            }
            (Some(previous), None) => {
                changes.push(DiagnosticEpisodeChange {
                    identity: previous.identity,
                    kind: DiagnosticEpisodeChangeKind::Resolved,
                    at,
                    before: Some(Arc::clone(&previous.current)),
                    after: None,
                    cause,
                });
                active.remove(&key);
            }
            // SPEC: docs/specs/contracts/persistent-diagnostic-episodes.yaml "material-change-versus-reevaluation"
            // Retain the last material problem and its owning view; time alone creates no change.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::NodeKey;
    use crate::metadata::DiagnosticMeta;
    use crate::{LevelSetResetConfig, NetworkBuilder, RuntimePolicy, TimeDomainId, Transaction};
    use core::marker::PhantomData;

    fn context() -> (crate::Machine<()>, ActiveDiagnosticEpisode<()>) {
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
        let control = builder.constant(LogicLevel::High);
        builder
            .add_level_set_reset_latch(
                NodeKey::from_u128(3),
                control,
                control,
                LevelSetResetConfig::new(LogicLevel::Low, ConflictPolicy::RetainAndDiagnose),
                DiagnosticMeta::default(),
            )
            .unwrap();
        let compiled = builder
            .finish()
            .require_artifact()
            .unwrap()
            .compile()
            .require_artifact()
            .unwrap();
        let policy = RuntimePolicy::builder()
            .max_internal_reactions(100)
            .max_evaluated_operations(1000)
            .max_pending_events(100)
            .max_events_created_per_transaction(100)
            .max_required_provenance_growth(10000)
            .build()
            .unwrap();
        let mut machine = compiled.spawn(policy);
        machine
            .apply(Transaction::initialize(
                Time::from_ticks(1),
                machine.revision(),
                compiled.input_snapshot().finish().unwrap(),
            ))
            .unwrap();
        let active = machine.active_diagnostic_episodes().unwrap().remove(0);
        (machine, active)
    }
    fn problem(active: &ActiveDiagnosticEpisode<()>, at: u64, previous: LogicLevel) -> Problem<()> {
        let mut evidence = conflict_evidence(active.current()).unwrap().clone();
        evidence.at_ticks = at;
        evidence.previous = previous;
        Problem::new(
            active.current().primary().clone(),
            Vec::new(),
            ProblemEvidence::RuntimeLevelLatchConflictRetained {
                evidence,
                marker: PhantomData,
            },
        )
    }

    #[test]
    fn material_changes_are_distinct_from_repeated_detection_and_candidate_changes_are_isolated() {
        let (machine, first) = context();
        let original = machine.store.active_episodes.clone();
        let mut candidate = original.clone();
        let mut changes = Vec::new();
        let causes = BTreeMap::from([(first.condition.owner.clone(), first.cause)]);
        reconcile(
            &mut candidate,
            &mut changes,
            NetworkKey::from_u128(1),
            Time::from_ticks(2),
            vec![problem(&first, 2, LogicLevel::Low)],
            &causes,
            &first.provenance,
        );
        assert!(changes.is_empty());
        reconcile(
            &mut candidate,
            &mut changes,
            NetworkKey::from_u128(1),
            Time::from_ticks(3),
            vec![problem(&first, 3, LogicLevel::High)],
            &causes,
            &first.provenance,
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind(), DiagnosticEpisodeChangeKind::Changed);
        assert_eq!(changes[0].before(), Some(first.current()));
        let after = candidate.values().next().unwrap();
        assert_eq!(after.identity, first.identity);
        assert_eq!(after.began_at, first.began_at);
        assert_eq!(after.last_material_change, Time::from_ticks(3));
        assert_eq!(changes[0].after(), Some(after.current()));
        // Discarding the candidate (as on a later failed transaction phase) leaves all original evidence intact.
        drop(candidate);
        let unchanged = machine.store.active_episodes.values().next().unwrap();
        assert_eq!(unchanged.current(), first.current());
        assert_eq!(unchanged.cause, first.cause);
        assert_eq!(unchanged.last_material_change, Time::from_ticks(1));
    }

    #[test]
    fn episode_construction_rejects_non_episode_delivery_and_incoherent_controls_subjects() {
        let (_, first) = context();
        let original = conflict_evidence(first.current()).unwrap();
        for evidence in [
            ConflictEvidence {
                controls: ConflictControls::Pulse {
                    set: crate::signal::PulseCount::ONE,
                    reset: crate::signal::PulseCount::ONE,
                },
                ..original.clone()
            },
            ConflictEvidence {
                controls: ConflictControls::Level {
                    set: LogicLevel::Low,
                    reset: LogicLevel::High,
                },
                ..original.clone()
            },
            ConflictEvidence {
                policy: ConflictPolicy::SetDominant,
                ..original.clone()
            },
            ConflictEvidence {
                node: NodeEvidence::Node(NodeKey::from_u128(999)),
                ..original.clone()
            },
        ] {
            let bad = Problem::<()>::new(
                first.current().primary().clone(),
                Vec::new(),
                ProblemEvidence::RuntimeLevelLatchConflictRetained {
                    evidence,
                    marker: PhantomData,
                },
            );
            assert!(DiagnosticConditionKey::from_problem(&bad).is_none());
        }
        let qualified_evidence = ConflictEvidence {
            node: NodeEvidence::Qualified {
                instances: vec![crate::key::ModuleInstanceKey::from_u128(10)],
                node: NodeKey::from_u128(20),
            },
            ..original.clone()
        };
        let mismatched_qualified = Problem::<()>::new(
            SubjectRef::QualifiedNode(QualifiedNodeRef::new(
                vec![crate::key::ModuleInstanceKey::from_u128(11)],
                NodeKey::from_u128(20),
            )),
            Vec::new(),
            ProblemEvidence::RuntimeLevelLatchConflictRetained {
                evidence: qualified_evidence,
                marker: PhantomData,
            },
        );
        assert!(DiagnosticConditionKey::from_problem(&mismatched_qualified).is_none());
        for evidence in [
            ProblemEvidence::<()>::RuntimeLevelLatchConflictRejected {
                evidence: original.clone(),
                marker: PhantomData,
            },
            ProblemEvidence::RuntimePulseLatchConflictRetained {
                evidence: original.clone(),
                marker: PhantomData,
            },
        ] {
            let bad = Problem::new(first.current().primary().clone(), Vec::new(), evidence);
            assert!(DiagnosticConditionKey::from_problem(&bad).is_none());
        }
        assert_eq!(
            DiagnosticConditionKey::from_problem(&problem(&first, 9, LogicLevel::High)),
            Some(first.condition.clone())
        );
        assert_ne!(
            DiagnosticEpisodeId::derive(NetworkKey::from_u128(2), &first.condition),
            first.identity
        );
    }
}
