//! Semantic comparison for opaque references belonging to distinct owners.
use mossignal::{CauseInspection, CauseRef, ProvenanceView};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticCause {
    fact: String,
    supporters: Vec<Arc<SemanticCause>>,
    contributions: Vec<(String, u64, Arc<SemanticCause>)>,
}

pub fn semantic<D>(view: &ProvenanceView<D>, root: CauseRef) -> Arc<SemanticCause> {
    fn visit<D>(
        view: &ProvenanceView<D>,
        cause: CauseRef,
        memo: &mut BTreeMap<CauseRef, Arc<SemanticCause>>,
    ) -> Arc<SemanticCause> {
        if let Some(value) = memo.get(&cause) {
            return Arc::clone(value);
        }
        let record = view.inspect(cause).unwrap();
        let (fact, parents, counted) = match record {
            CauseInspection::TopologyChange {
                at,
                revision,
                base,
                target,
                supporters,
            } => (
                format!("patch:{at:?}:{revision:?}:{base:?}:{target:?}"),
                supporters,
                &[][..],
            ),
            CauseInspection::Migration {
                subject,
                rule,
                supporters,
            } => (format!("migration:{subject:?}:{rule}"), supporters, &[][..]),
            CauseInspection::Checkpoint { fact, supporters } => {
                (format!("checkpoint:{fact:?}"), supporters, &[][..])
            }
            CauseInspection::InitializationTransaction { at, revision } => {
                (format!("initialize:{at:?}:{revision:?}"), &[][..], &[][..])
            }
            CauseInspection::ReadyTransaction { at, revision } => {
                (format!("advance:{at:?}:{revision:?}"), &[][..], &[][..])
            }
            CauseInspection::ExternalObservation {
                input,
                value,
                stamp,
            } => (
                format!("level:{input:?}:{value:?}:{stamp:?}"),
                &[][..],
                &[][..],
            ),
            CauseInspection::ExternalPulseObservation {
                input,
                count,
                stamp,
            } => (
                format!("pulse:{input:?}:{count:?}:{stamp:?}"),
                &[][..],
                &[][..],
            ),
            CauseInspection::PendingPulseDelay {
                event,
                owner,
                stimulus,
                origin,
                deadline,
                count,
                revision,
                supporters,
            } => (
                format!(
                    "pulse-delay:{event:?}:{owner:?}:{stimulus:?}:{origin:?}:{deadline:?}:{count:?}:{revision:?}"
                ),
                supporters,
                &[][..],
            ),
            CauseInspection::PendingTransportDelay {
                event,
                owner,
                stimulus,
                origin,
                deadline,
                target,
                revision,
                supporters,
            } => (
                format!(
                    "transport:{event:?}:{owner:?}:{stimulus:?}:{origin:?}:{deadline:?}:{target:?}:{revision:?}"
                ),
                supporters,
                &[][..],
            ),
            CauseInspection::PendingInertialDelay {
                event,
                owner,
                stimulus,
                origin,
                deadline,
                target,
                revision,
                supporters,
            } => (
                format!(
                    "inertial:{event:?}:{owner:?}:{stimulus:?}:{origin:?}:{deadline:?}:{target:?}:{revision:?}"
                ),
                supporters,
                &[][..],
            ),
            CauseInspection::PendingPeriodicBoundary {
                event,
                owner,
                stimulus,
                origin,
                deadline,
                anchor,
                ordinal,
                first_emission,
                reenable_phase,
                revision,
                supporters,
            } => (
                format!(
                    "periodic:{event:?}:{owner:?}:{stimulus:?}:{origin:?}:{deadline:?}:{anchor:?}:{ordinal}:{first_emission:?}:{reenable_phase:?}:{revision:?}"
                ),
                supporters,
                &[][..],
            ),
            CauseInspection::Derived {
                subject,
                supporters,
            } => (format!("derived:{subject:?}"), supporters, &[][..]),
            CauseInspection::PulseDerived {
                subject,
                result,
                supporters,
                contributions,
            } => (
                format!("pulse-derived:{subject:?}:{result:?}"),
                supporters,
                contributions,
            ),
            CauseInspection::PulseControlledLevel {
                subject,
                result,
                supporters,
                contributions,
            } => (
                format!("pulse-level:{subject:?}:{result:?}"),
                supporters,
                contributions,
            ),
            _ => panic!("new cause inspection variants need semantic comparison"),
        };
        let mut supporters: Vec<_> = parents
            .iter()
            .map(|parent| visit(view, *parent, memo))
            .collect();
        supporters.sort();
        let mut contributions: Vec<_> = counted
            .iter()
            .map(|value| {
                (
                    format!("{:?}", value.port()),
                    value.count().get(),
                    visit(view, value.cause(), memo),
                )
            })
            .collect();
        // Preserve relative order for repeated ports, matching the canonical relation law.
        contributions.sort_by(|a, b| a.0.cmp(&b.0));
        let value = Arc::new(SemanticCause {
            fact,
            supporters,
            contributions,
        });
        memo.insert(cause, Arc::clone(&value));
        value
    }
    visit(view, root, &mut BTreeMap::new())
}
