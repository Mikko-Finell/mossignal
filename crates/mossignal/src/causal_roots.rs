//! Current ownership and stable causal-role bindings, distinct from allocations.

use crate::identity::Cbor;
use crate::{Machine, transaction::CauseRef};

pub(crate) fn machine<D>(machine: &Machine<D>) -> Vec<CauseRef> {
    let store = &machine.store;
    let mut roots = store.operation_causes.clone();
    roots.extend(store.input_causes.values().copied());
    roots.extend(store.output_causes.values().copied());
    for causes in [
        &store.edge_observation_causes,
        &store.toggle_inversion_causes,
        &store.establishment_causes,
        &store.transport_transition_causes,
        &store.inertial_cancellation_causes,
        &store.periodic_anchor_causes,
        &store.periodic_cancellation_causes,
    ] {
        roots.extend(causes.values().copied());
    }
    roots.extend(
        store
            .pending_events
            .values()
            .flatten()
            .map(|event| event.identity().5),
    );
    roots.extend(
        store
            .standard_causes
            .values()
            .flat_map(|facts| facts.retained_causes()),
    );
    roots.extend(
        store
            .active_episodes
            .values()
            .map(|episode| episode.cause()),
    );
    roots.sort();
    roots.dedup();
    roots
}

pub(crate) fn module_subject(module: &crate::QualifiedModuleRef) -> Vec<u8> {
    let mut writer = Cbor::default();
    writer.variant_start("module");
    writer.array_start(module.instances().len());
    for key in module.instances() {
        writer.key(key.as_u128());
    }
    writer.finish()
}

pub(crate) type Binding = (Vec<u8>, &'static str, CauseRef);

pub(crate) fn bindings<D>(machine: &Machine<D>, snapshot: bool) -> Vec<Binding> {
    if !machine.is_initialized() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for (operation, subject) in machine.compiled.current_cause_slots() {
        rows.push((
            subject,
            "current",
            machine.store.operation_causes[operation],
        ));
    }
    for (module, facts) in &machine.store.standard_causes {
        for (role, cause) in [
            ("latest_reset", facts.latest_reset),
            ("latest_toggle", facts.latest_toggle),
            ("latest_capture", facts.latest_capture),
        ] {
            if let Some(cause) = cause {
                rows.push((module_subject(module), role, cause));
            }
        }
    }
    if !snapshot {
        for (key, cause) in &machine.store.input_causes {
            let mut subject = Cbor::default();
            subject.variant_start("external_input");
            subject.key(key.as_u128());
            rows.push((subject.finish(), "origin", *cause));
        }
        for (key, cause) in &machine.store.output_causes {
            let mut subject = Cbor::default();
            subject.variant_start("external_output");
            subject.key(key.as_u128());
            rows.push((subject.finish(), "baseline", *cause));
        }
        for (role, causes) in [
            ("edge_observation", &machine.store.edge_observation_causes),
            ("toggle_inversion", &machine.store.toggle_inversion_causes),
            ("establishment", &machine.store.establishment_causes),
            (
                "transport_transition",
                &machine.store.transport_transition_causes,
            ),
            (
                "inertial_cancellation",
                &machine.store.inertial_cancellation_causes,
            ),
            ("periodic_anchor", &machine.store.periodic_anchor_causes),
            (
                "periodic_cancellation",
                &machine.store.periodic_cancellation_causes,
            ),
        ] {
            for (node, cause) in causes {
                rows.push((
                    crate::state_digest::encode_stable_owner(&machine.compiled.stable_owner(*node)),
                    role,
                    *cause,
                ));
            }
        }
    }
    rows.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    rows
}
