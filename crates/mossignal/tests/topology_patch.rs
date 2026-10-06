//! Structural preparation of one topology replacement.
//!
//! Preparation reads the base topology and the patch. It does not read or
//! mutate a running machine.

use mossignal::authored::{ConnectionDef, ConnectionEndpoint, ExternalInputDef, ExternalOutputDef};
use mossignal::diagnostics::SubjectRef;
use mossignal::key::{
    AnyInPortKey, ExternalInputKey, ExternalOutputKey, InPortKey, ModuleInputKey,
    ModuleInstanceKey, NetworkKey, NodeKey, OutPortKey,
};
use mossignal::metadata::DiagnosticMeta;
use mossignal::signal::{Level, LogicLevel};
use mossignal::standard::StandardInternalCategory;
use mossignal::time::{NonZeroSpan, Time};
use mossignal::{
    ArtifactInvalidation, ConditionalArm, Continuity, FirstEmissionPolicy, InertialDelayConfig,
    InputValuationPlan, KeyedModuleInput, LossClass, Machine, MachineSnapshot,
    ModuleMigrationDirective, ModuleNodeMigrationDirective, NetworkBuilder, NodeMigrationDirective,
    OutputBaselinePlan, PendingArm, PendingWorkRule, PeriodicConfig, PeriodicMigration,
    PulseDelayConfig, PulseDelayMigration, ReenablePhasePolicy, RuntimePolicy, StateCompatibility,
    StructuralSubjectRef, SubjectReassociation, TimeDomainId, ToggleConfig, Transaction,
};

#[derive(Debug, PartialEq, Eq)]
enum Domain {}

fn policy() -> RuntimePolicy {
    RuntimePolicy::builder()
        .max_internal_reactions(1_000)
        .max_evaluated_operations(100_000)
        .max_pending_events(1_000)
        .max_events_created_per_transaction(10_000)
        .max_required_provenance_growth(100_000)
        .build()
        .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
}

fn named(name: &str) -> DiagnosticMeta {
    DiagnosticMeta {
        name: Some(name.to_owned()),
        ..DiagnosticMeta::default()
    }
}

fn compile(builder: NetworkBuilder<Domain>) -> mossignal::CompiledNetwork<Domain> {
    builder
        .finish()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must validate: {failure:?}"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|failure| panic!("network must compile: {failure:?}"))
}

fn codes<T>(report: &mossignal::diagnostics::Report<T, Domain>) -> Vec<&str> {
    report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.problem().code().as_str())
        .collect()
}

fn not_gate() -> (
    mossignal::CompiledNetwork<Domain>,
    ExternalInputKey<Level>,
    ExternalOutputKey<Level>,
) {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(7), TimeDomainId::from_u128(11));
    let input = ExternalInputKey::from_u128(1);
    let signal = builder
        .add_level_input(input, DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let inverted = builder
        .add_not_with_ports(
            NodeKey::from_u128(2),
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            signal,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("Not must author: {failure:?}"))
        .into_outputs();
    let output = ExternalOutputKey::from_u128(5);
    builder
        .add_level_output(output, inverted, named("out"))
        .unwrap_or_else(|failure| panic!("level output must author: {failure:?}"));
    (compile(builder), input, output)
}

fn level_snapshot(
    compiled: &mossignal::CompiledNetwork<Domain>,
    input: ExternalInputKey<Level>,
    level: LogicLevel,
) -> mossignal::InputSnapshot<Domain> {
    compiled
        .input_snapshot()
        .set(input, level)
        .unwrap_or_else(|failure| panic!("level observation must bind: {failure}"))
        .finish()
        .unwrap_or_else(|failure| panic!("snapshot must finish: {failure}"))
}

fn assert_unchanged(before: &MachineSnapshot<Domain>, machine: &Machine<Domain>) {
    assert_eq!(machine.snapshot(), *before);
    assert_eq!(machine.revision(), before.revision());
}

#[test]
fn preparation_depends_only_on_topology_and_patch() {
    let (compiled, input, _) = not_gate();
    let fresh = compiled.spawn(policy());
    let mut settled = compiled.spawn(policy());
    settled
        .apply(Transaction::initialize(
            Time::from_ticks(0),
            settled.revision(),
            level_snapshot(&compiled, input, LogicLevel::High),
        ))
        .unwrap_or_else(|failure| panic!("initialization must apply: {failure:?}"));
    let fresh_before = fresh.snapshot();
    let settled_before = settled.snapshot();
    let fresh_status = fresh.status();
    let settled_status = settled.status();
    let edit = named("renamed");
    let fresh_patch = fresh
        .patch()
        .set_diagnostic_meta(
            StructuralSubjectRef::Network(compiled.network_key()),
            edit.clone(),
        )
        .unwrap_or_else(|failure| panic!("metadata edit must build: {failure:?}"))
        .finish();
    let settled_patch = settled
        .patch()
        .set_diagnostic_meta(StructuralSubjectRef::Network(compiled.network_key()), edit)
        .unwrap_or_else(|failure| panic!("metadata edit must build: {failure:?}"))
        .finish();
    let fresh_report = fresh.prepare_patch(fresh_patch);
    let settled_report = settled.prepare_patch(settled_patch);
    let fresh_prepared = fresh_report.artifact().cloned().unwrap_or_else(|| {
        panic!(
            "fresh metadata patch must prepare: {:?}",
            codes(&fresh_report)
        )
    });
    let settled_prepared = settled_report.artifact().cloned().unwrap_or_else(|| {
        panic!(
            "settled metadata patch must prepare: {:?}",
            codes(&settled_report)
        )
    });
    assert_eq!(codes(&fresh_report), codes(&settled_report));
    assert_eq!(fresh_prepared.static_plan(), settled_prepared.static_plan());
    assert_eq!(
        fresh_prepared.resulting_fingerprint(),
        settled_prepared.resulting_fingerprint()
    );
    assert_eq!(
        fresh_prepared.proposed_revision(),
        settled_prepared.proposed_revision()
    );
    assert_eq!(
        fresh_prepared.base_fingerprint(),
        fresh_prepared.resulting_fingerprint()
    );
    assert_ne!(
        fresh_prepared.base_revision(),
        fresh_prepared.proposed_revision()
    );
    let cloned = fresh_prepared.clone();
    assert_eq!(cloned.static_plan(), fresh_prepared.static_plan());
    assert_unchanged(&fresh_before, &fresh);
    assert_unchanged(&settled_before, &settled);
    assert_eq!(fresh.status(), fresh_status);
    assert_eq!(settled.status(), settled_status);
    assert!(
        fresh_prepared
            .static_plan()
            .invalidated()
            .contains(&ArtifactInvalidation::ResolvedHandles)
    );
    assert!(
        fresh_prepared
            .static_plan()
            .invalidated()
            .contains(&ArtifactInvalidation::CompiledInspectionPlans)
    );
    assert!(
        fresh_prepared
            .static_plan()
            .invalidated()
            .contains(&ArtifactInvalidation::OldRevisionPreparedPatches)
    );
    assert!(
        !fresh_prepared
            .static_plan()
            .invalidated()
            .contains(&ArtifactInvalidation::OldSchemaInputArtifacts)
    );
}

#[test]
fn operation_order_agrees_on_the_normalized_result() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(1), TimeDomainId::from_u128(2));
    builder
        .add_constant(
            NodeKey::from_u128(3),
            LogicLevel::Low,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("constant must author: {failure:?}"));
    builder
        .add_constant(
            NodeKey::from_u128(4),
            LogicLevel::High,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("constant must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let alpha = NodeKey::from_u128(3);
    let beta = NodeKey::from_u128(4);
    let forward = machine
        .prepare_patch(
            machine
                .patch()
                .set_diagnostic_meta(StructuralSubjectRef::Node(alpha), named("alpha"))
                .unwrap_or_else(|failure| panic!("alpha metadata must build: {failure:?}"))
                .set_diagnostic_meta(StructuralSubjectRef::Node(beta), named("beta"))
                .unwrap_or_else(|failure| panic!("beta metadata must build: {failure:?}"))
                .finish(),
        )
        .require_artifact()
        .unwrap_or_else(|failure| panic!("forward patch must prepare: {failure:?}"));
    let reverse = machine
        .prepare_patch(
            machine
                .patch()
                .set_diagnostic_meta(StructuralSubjectRef::Node(beta), named("beta"))
                .unwrap_or_else(|failure| panic!("beta metadata must build: {failure:?}"))
                .set_diagnostic_meta(StructuralSubjectRef::Node(alpha), named("alpha"))
                .unwrap_or_else(|failure| panic!("alpha metadata must build: {failure:?}"))
                .finish(),
        )
        .require_artifact()
        .unwrap_or_else(|failure| panic!("reverse patch must prepare: {failure:?}"));
    assert_eq!(
        forward.operations().collect::<Vec<_>>(),
        reverse.operations().collect::<Vec<_>>()
    );
    assert_eq!(
        forward.resulting_fingerprint(),
        reverse.resulting_fingerprint()
    );
    assert_eq!(forward.static_plan(), reverse.static_plan());
}

#[test]
fn output_reassociation_is_explicit() {
    let (compiled, _, output) = not_gate();
    let machine = compiled.spawn(policy());
    let existing = compiled
        .graph()
        .external_outputs()
        .iter()
        .find(|item| item.key() == output.into())
        .unwrap_or_else(|| panic!("level output must be retained"));
    let source = existing.source();
    let renamed = ExternalOutputKey::<Level>::from_u128(50);
    let replacement = ExternalOutputDef::new(renamed.into(), source, named("out"));
    let shared_name = machine
        .patch()
        .remove_external_output(output.into())
        .unwrap_or_else(|failure| panic!("output removal must build: {failure:?}"))
        .add_external_output(replacement.clone())
        .unwrap_or_else(|failure| panic!("output addition must build: {failure:?}"))
        .finish();
    let explicit = machine
        .patch()
        .remove_external_output(output.into())
        .unwrap_or_else(|failure| panic!("output removal must build: {failure:?}"))
        .add_external_output(replacement)
        .unwrap_or_else(|failure| panic!("output addition must build: {failure:?}"))
        .reassociate(SubjectReassociation::ExternalOutput {
            from: output.into(),
            to: renamed.into(),
        })
        .unwrap_or_else(|failure| panic!("reassociation must build: {failure:?}"))
        .finish();
    let shared = machine
        .prepare_patch(shared_name)
        .require_artifact()
        .unwrap_or_else(|failure| panic!("renamed output must prepare: {failure:?}"));
    let associated = machine
        .prepare_patch(explicit)
        .require_artifact()
        .unwrap_or_else(|failure| panic!("reassociated output must prepare: {failure:?}"));
    let shared_plan = shared.static_plan().external_outputs();
    assert!(shared_plan.iter().any(|plan| {
        plan.continuity() == Continuity::Removed && plan.source() == Some(output.into())
    }));
    assert!(shared_plan.iter().any(|plan| {
        plan.continuity() == Continuity::Added && plan.target() == Some(renamed.into())
    }));
    assert!(
        shared_plan
            .iter()
            .all(|plan| plan.continuity() != Continuity::Reassociated)
    );
    assert!(
        associated
            .static_plan()
            .external_outputs()
            .iter()
            .any(|plan| {
                plan.continuity() == Continuity::Reassociated
                    && plan.source() == Some(output.into())
                    && plan.target() == Some(renamed.into())
                    && plan.baseline() == OutputBaselinePlan::CarryAsEvidence
            })
    );
    assert!(associated.static_plan().rebinding().iter().any(|notice| {
        notice.source() == Some(&SubjectRef::ExternalOutput(output.into()))
            && notice.target() == Some(&SubjectRef::ExternalOutput(renamed.into()))
    }));
}

#[test]
fn malformed_connection_stays_with_ordinary_validation() {
    let (compiled, input, _) = not_gate();
    let machine = compiled.spawn(policy());
    let patch = machine
        .patch()
        .add_connection(ConnectionDef::new(
            mossignal::key::ConnectionKey::from_u128(99),
            ConnectionEndpoint::external_input(input.into()),
            ConnectionEndpoint::node_input(AnyInPortKey::Level(InPortKey::from_u128(12345))),
            DiagnosticMeta::default(),
        ))
        .unwrap_or_else(|failure| panic!("connection claim must build: {failure:?}"))
        .finish();
    let report = machine.prepare_patch(patch);
    assert!(report.artifact().is_none());
    let found = codes(&report);
    assert!(found.iter().any(|code| code.starts_with("validation.")));
    assert!(
        found
            .iter()
            .all(|code| !code.starts_with("reconfiguration."))
    );
    assert_eq!(machine.revision(), compiled.spawn(policy()).revision());
}

#[test]
fn empty_patch_yields_no_artifact() {
    let (compiled, _, _) = not_gate();
    let machine = compiled.spawn(policy());
    let builder = machine.patch();
    assert_eq!(builder.base_revision(), machine.revision());
    assert_eq!(builder.base_fingerprint(), compiled.fingerprint());
    let report = machine.prepare_patch(builder.finish());
    assert!(report.artifact().is_none());
    assert_eq!(codes(&report), ["reconfiguration.empty_patch"]);
}

#[test]
fn reset_reports_unavoidable_stored_level_loss() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(8), TimeDomainId::from_u128(9));
    let input = builder
        .add_pulse_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    let key = NodeKey::from_u128(2);
    builder
        .add_toggle_with_ports(
            key,
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            input,
            ToggleConfig::new(LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("toggle must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let node = compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == key)
        .unwrap_or_else(|| panic!("toggle must be retained"))
        .clone();
    let prepared = machine
        .patch()
        .replace_node(key, node, NodeMigrationDirective::Reset)
        .unwrap_or_else(|failure| panic!("reset replacement must build: {failure:?}"))
        .finish();
    let report = machine.prepare_patch(prepared);
    let prepared = report
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("reset must prepare: {:?}", codes(&report)));
    assert!(
        prepared
            .static_plan()
            .potential_losses()
            .iter()
            .any(|loss| {
                loss.class() == LossClass::Unavoidable
                    && loss.fact() == "stored_level"
                    && loss.rule() == "reset"
            })
    );
    assert!(prepared.static_plan().subjects().iter().any(|plan| {
        plan.continuity() == Continuity::Preserved && plan.source() == Some(&SubjectRef::Node(key))
    }));
    assert_ne!(prepared.base_revision(), prepared.proposed_revision());
}

#[test]
fn removed_level_output_reports_conditional_baseline_loss() {
    let (compiled, _, output) = not_gate();
    let machine = compiled.spawn(policy());
    let report = machine.prepare_patch(
        machine
            .patch()
            .remove_external_output(output.into())
            .unwrap_or_else(|failure| panic!("output removal must build: {failure:?}"))
            .finish(),
    );
    let prepared = report
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("output removal must prepare: {:?}", codes(&report)));
    assert!(
        prepared
            .static_plan()
            .potential_losses()
            .iter()
            .any(|loss| {
                loss.class() == LossClass::Conditional
                    && loss.fact() == "level_baseline"
                    && loss.rule() == "removed"
                    && loss.subject() == &SubjectRef::ExternalOutput(output.into())
            })
    );
    assert!(
        prepared
            .static_plan()
            .external_outputs()
            .iter()
            .any(|plan| {
                plan.continuity() == Continuity::Removed
                    && plan.baseline() == OutputBaselinePlan::Remove
            })
    );
}

#[test]
fn new_level_input_must_be_established() {
    let (compiled, _, _) = not_gate();
    let machine = compiled.spawn(policy());
    let added = ExternalInputKey::from_u128(40);
    let report = machine.prepare_patch(
        machine
            .patch()
            .add_external_input(ExternalInputDef::new(
                added.into(),
                DiagnosticMeta::default(),
            ))
            .unwrap_or_else(|failure| panic!("input addition must build: {failure:?}"))
            .finish(),
    );
    let prepared = report
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("input addition must prepare: {:?}", codes(&report)));
    assert!(prepared.static_plan().external_inputs().iter().any(|plan| {
        plan.continuity() == Continuity::Added
            && plan.target() == Some(added.into())
            && plan.valuation() == InputValuationPlan::Establish
    }));
    assert!(
        prepared
            .static_plan()
            .invalidated()
            .contains(&ArtifactInvalidation::SchemaBoundBindingProjectors)
    );
    assert!(
        prepared
            .static_plan()
            .invalidated()
            .contains(&ArtifactInvalidation::OldSchemaInputArtifacts)
    );
    prepared
        .input_snapshot()
        .set(ExternalInputKey::from_u128(1), LogicLevel::Low)
        .unwrap_or_else(|failure| panic!("preserved input must bind: {failure}"))
        .set(added, LogicLevel::High)
        .unwrap_or_else(|failure| panic!("new input must bind: {failure}"))
        .finish()
        .unwrap_or_else(|failure| panic!("target snapshot must finish: {failure}"));
}

#[test]
fn standard_inertial_delay_change_records_both_arms() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(12), TimeDomainId::from_u128(13));
    let input = builder
        .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let key = NodeKey::from_u128(2);
    let delay = NonZeroSpan::from_ticks(3)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"));
    builder
        .add_inertial_delay_with_ports(
            key,
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            input,
            InertialDelayConfig::new(delay, LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("inertial delay must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let node = compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == key)
        .unwrap_or_else(|| panic!("inertial delay must be retained"))
        .clone();
    let (node_key, _, ports, meta) = node.into_parts();
    let longer = NonZeroSpan::from_ticks(4)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"));
    let replacement = mossignal::authored::NodeDef::new(
        node_key,
        mossignal::authored::NodeKind::inertial_delay(longer, LogicLevel::Low),
        ports,
        meta,
    );
    let report = machine.prepare_patch(
        machine
            .patch()
            .replace_node(key, replacement, NodeMigrationDirective::Standard)
            .unwrap_or_else(|failure| panic!("delay replacement must build: {failure:?}"))
            .finish(),
    );
    let prepared = report
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("inertial patch must prepare: {:?}", codes(&report)));
    assert!(
        codes(&report)
            .iter()
            .all(|code| *code != "reconfiguration.incomplete_temporal_migration_policy")
    );
    let rule = prepared
        .static_plan()
        .event_rules()
        .iter()
        .find(|rule| rule.subject() == &SubjectRef::Node(key))
        .unwrap_or_else(|| panic!("inertial node must have one pending rule"));
    assert_eq!(
        rule.rule(),
        &PendingWorkRule::Conditional {
            fact: "inertial_candidate",
            when_clear: PendingArm::PreserveDeadline,
            when_set: PendingArm::Reject,
        }
    );
    assert!(
        prepared
            .static_plan()
            .potential_losses()
            .iter()
            .any(|loss| {
                loss.class() == LossClass::Conditional
                    && loss.fact() == "inertial_candidate"
                    && loss.rule() == "standard"
            })
    );
}

#[test]
fn standard_pulse_delay_change_preserves_deadlines_without_loss() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(14), TimeDomainId::from_u128(15));
    let input = builder
        .add_pulse_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    let key = NodeKey::from_u128(2);
    let delay = NonZeroSpan::from_ticks(3)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"));
    builder
        .add_pulse_delay_with_ports(
            key,
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            input,
            PulseDelayConfig::new(delay),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("pulse delay must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let node = compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == key)
        .unwrap_or_else(|| panic!("pulse delay must be retained"))
        .clone();
    let (node_key, _, ports, meta) = node.into_parts();
    let longer = NonZeroSpan::from_ticks(5)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"));
    let replacement = mossignal::authored::NodeDef::new(
        node_key,
        mossignal::authored::NodeKind::pulse_delay(longer),
        ports,
        meta,
    );
    let report = machine.prepare_patch(
        machine
            .patch()
            .replace_node(key, replacement, NodeMigrationDirective::Standard)
            .unwrap_or_else(|failure| panic!("delay replacement must build: {failure:?}"))
            .finish(),
    );
    let prepared = report
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("pulse delay patch must prepare: {:?}", codes(&report)));
    assert!(prepared.static_plan().potential_losses().is_empty());
    let rule = prepared
        .static_plan()
        .event_rules()
        .iter()
        .find(|rule| rule.subject() == &SubjectRef::Node(key))
        .unwrap_or_else(|| panic!("pulse delay must have one pending rule"));
    assert_eq!(rule.rule(), &PendingWorkRule::PreserveDeadline);
    assert!(matches!(
        prepared
            .static_plan()
            .subjects()
            .iter()
            .find(|plan| plan.source() == Some(&SubjectRef::Node(key)))
            .and_then(|plan| plan.state()),
        Some(StateCompatibility::Preserve) | None
    ));
}

#[test]
fn exactly_preserves_internal_roles_without_state_loss() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(16), TimeDomainId::from_u128(17));
    let source = builder
        .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let instance = ModuleInstanceKey::from_u128(3);
    let input = ModuleInputKey::from_u128(10);
    builder
        .add_exactly(
            instance,
            0,
            [KeyedModuleInput { key: input, source }],
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("Exactly must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let preserved = machine
        .prepare_patch(
            machine
                .patch()
                .set_diagnostic_meta(StructuralSubjectRef::Module(instance), named("counter"))
                .unwrap_or_else(|failure| panic!("instance metadata must build: {failure:?}"))
                .finish(),
        )
        .require_artifact()
        .unwrap_or_else(|failure| panic!("Exactly metadata must prepare: {failure:?}"));
    let module = preserved
        .static_plan()
        .modules()
        .iter()
        .find(|module| module.source() == Some(instance))
        .unwrap_or_else(|| panic!("Exactly instance must be classified"));
    assert_eq!(module.continuity(), Continuity::Preserved);
    assert!(
        module
            .internals()
            .iter()
            .all(|role| role.continuity() == Continuity::Preserved)
    );
    assert!(
        preserved
            .static_plan()
            .potential_losses()
            .iter()
            .all(|loss| loss.class() != LossClass::Unavoidable)
    );

    let mut other =
        NetworkBuilder::with_key(NetworkKey::from_u128(16), TimeDomainId::from_u128(17));
    let other_source = other
        .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    other
        .add_exactly(
            instance,
            1,
            [KeyedModuleInput {
                key: input,
                source: other_source,
            }],
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("replacement Exactly must author: {failure:?}"));
    let other = compile(other);
    let replacement = other
        .graph()
        .module_instances()
        .iter()
        .find(|module| module.key() == instance)
        .unwrap_or_else(|| panic!("replacement Exactly must be retained"))
        .clone();
    let replaced = machine
        .prepare_patch(
            machine
                .patch()
                .replace_module_instance(instance, replacement, ModuleMigrationDirective::Standard)
                .unwrap_or_else(|failure| panic!("threshold replacement must build: {failure:?}"))
                .finish(),
        )
        .require_artifact()
        .unwrap_or_else(|failure| panic!("threshold replacement must prepare: {failure:?}"));
    assert!(
        replaced
            .static_plan()
            .potential_losses()
            .iter()
            .all(|loss| loss.class() != LossClass::Unavoidable)
    );

    let internal = compiled
        .graph()
        .module_instances()
        .iter()
        .find(|module| module.key() == instance)
        .and_then(|module| module.module().standard_declaration())
        .and_then(|declaration| {
            declaration
                .internal_roles()
                .find(|role| role.category() == StandardInternalCategory::Node)
        })
        .map(|role| NodeKey::from_u128(role.key()))
        .unwrap_or_else(|| panic!("Exactly must expose an internal node role"));
    let rejected = machine.prepare_patch(
        machine
            .patch()
            .remove_node(internal)
            .unwrap_or_else(|failure| panic!("internal removal must build: {failure:?}"))
            .finish(),
    );
    assert!(rejected.artifact().is_none());
    let rejected_codes = codes(&rejected);
    assert!(rejected_codes.contains(&"standard_module.noncanonical_internal_edit"));
    assert!(!rejected_codes.contains(&"reconfiguration.unknown_base_subject"));
}

#[test]
fn foreign_topology_is_rejected() {
    let (compiled, _, _) = not_gate();
    let machine = compiled.spawn(policy());
    let mut other =
        NetworkBuilder::with_key(NetworkKey::from_u128(90), TimeDomainId::from_u128(91));
    other
        .add_constant(
            NodeKey::from_u128(3),
            LogicLevel::Low,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("constant must author: {failure:?}"));
    let other = compile(other);
    let patch = other
        .patch(machine.revision())
        .set_diagnostic_meta(
            StructuralSubjectRef::Network(other.network_key()),
            named("other"),
        )
        .unwrap_or_else(|failure| panic!("foreign metadata must build: {failure:?}"))
        .finish();
    let report = machine.prepare_patch(patch);
    assert!(report.artifact().is_none());
    assert_eq!(
        codes(&report),
        ["reconfiguration.base_fingerprint_mismatch"]
    );
    assert_eq!(machine.revision(), compiled.spawn(policy()).revision());
}

#[test]
fn restart_from_patch_time_retimes_every_group() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(21), TimeDomainId::from_u128(22));
    let input = builder
        .add_pulse_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("pulse input must author: {failure:?}"));
    let key = NodeKey::from_u128(2);
    let delay = NonZeroSpan::from_ticks(3)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"));
    builder
        .add_pulse_delay_with_ports(
            key,
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            input,
            PulseDelayConfig::new(delay),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("pulse delay must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let node = compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == key)
        .unwrap_or_else(|| panic!("pulse delay must be retained"))
        .clone();
    let (node_key, _, ports, meta) = node.into_parts();
    let longer = NonZeroSpan::from_ticks(8)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"));
    let replacement = mossignal::authored::NodeDef::new(
        node_key,
        mossignal::authored::NodeKind::pulse_delay(longer),
        ports,
        meta,
    );
    let report = machine.prepare_patch(
        machine
            .patch()
            .replace_node(
                key,
                replacement,
                NodeMigrationDirective::PulseDelay(PulseDelayMigration::RestartFromPatchTime),
            )
            .unwrap_or_else(|failure| panic!("restart replacement must build: {failure:?}"))
            .finish(),
    );
    let prepared = report
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("restart must prepare: {:?}", codes(&report)));
    assert!(
        codes(&report)
            .iter()
            .all(|code| *code != "reconfiguration.incomplete_temporal_migration_policy")
    );
    let rule = prepared
        .static_plan()
        .event_rules()
        .iter()
        .find(|rule| rule.subject() == &SubjectRef::Node(key))
        .unwrap_or_else(|| panic!("restarted pulse delay must have one pending rule"));
    assert_eq!(rule.rule(), &PendingWorkRule::RecomputeDeadline);
    assert!(
        prepared
            .static_plan()
            .potential_losses()
            .iter()
            .any(|loss| {
                loss.class() == LossClass::Conditional
                    && loss.fact() == "elapsed_wait"
                    && loss.rule() == "restart_from_patch_time"
            })
    );
}

#[test]
fn reanchor_and_cancel_assign_total_periodic_outcomes() {
    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(23), TimeDomainId::from_u128(24));
    let enable = builder
        .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("enable input must author: {failure:?}"));
    let key = NodeKey::from_u128(2);
    let period = NonZeroSpan::from_ticks(5)
        .unwrap_or_else(|failure| panic!("period must be nonzero: {failure}"));
    let config = PeriodicConfig::new(
        period,
        FirstEmissionPolicy::Immediate,
        ReenablePhasePolicy::PreservePhase,
    );
    builder
        .add_periodic_with_ports(
            key,
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            enable,
            config,
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("periodic must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let node = compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == key)
        .unwrap_or_else(|| panic!("periodic node must be retained"))
        .clone();
    let prepare = |migration: PeriodicMigration| {
        let report = machine.prepare_patch(
            machine
                .patch()
                .replace_node(
                    key,
                    node.clone(),
                    NodeMigrationDirective::Periodic(migration),
                )
                .unwrap_or_else(|failure| panic!("periodic replacement must build: {failure:?}"))
                .finish(),
        );
        report
            .artifact()
            .cloned()
            .unwrap_or_else(|| panic!("periodic directive must prepare: {:?}", codes(&report)))
    };
    let reanchored = prepare(PeriodicMigration::ReanchorAtPatchTime);
    let reanchor_rule = reanchored
        .static_plan()
        .event_rules()
        .iter()
        .find(|rule| rule.subject() == &SubjectRef::Node(key))
        .unwrap_or_else(|| panic!("reanchor must have one pending rule"));
    assert_eq!(reanchor_rule.rule(), &PendingWorkRule::RecomputeDeadline);
    assert!(reanchored.static_plan().subjects().iter().any(|plan| {
        plan.source() == Some(&SubjectRef::Node(key))
            && plan.state() == Some(&StateCompatibility::Preserve)
    }));
    assert!(
        reanchored
            .static_plan()
            .potential_losses()
            .iter()
            .any(|loss| {
                loss.class() == LossClass::Conditional && loss.fact() == "periodic_schedule"
            })
    );
    let cancelled = prepare(PeriodicMigration::CancelSchedule);
    let cancel_rule = cancelled
        .static_plan()
        .event_rules()
        .iter()
        .find(|rule| rule.subject() == &SubjectRef::Node(key))
        .unwrap_or_else(|| panic!("cancel must have one pending rule"));
    assert_eq!(cancel_rule.rule(), &PendingWorkRule::Cancel);
    assert!(cancelled.static_plan().subjects().iter().any(|plan| {
        plan.source() == Some(&SubjectRef::Node(key))
            && plan.state() == Some(&StateCompatibility::Reset)
    }));
    assert!(
        cancelled
            .static_plan()
            .potential_losses()
            .iter()
            .any(|loss| loss.class() == LossClass::Unavoidable && loss.fact() == "periodic_enable")
    );
    assert!(
        cancelled
            .static_plan()
            .potential_losses()
            .iter()
            .any(|loss| {
                loss.class() == LossClass::Conditional && loss.fact() == "periodic_schedule"
            })
    );
}

#[test]
fn sample_hold_reset_to_is_a_settled_reset_predicate() {
    let instance = ModuleInstanceKey::from_u128(4);
    let hold = |reset_to: LogicLevel| {
        let mut builder =
            NetworkBuilder::with_key(NetworkKey::from_u128(25), TimeDomainId::from_u128(26));
        let value = builder
            .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("value input must author: {failure:?}"));
        let sample = builder
            .add_pulse_input(ExternalInputKey::from_u128(2), DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("sample input must author: {failure:?}"));
        let reset = builder
            .add_level_input(ExternalInputKey::from_u128(3), DiagnosticMeta::default())
            .unwrap_or_else(|failure| panic!("reset input must author: {failure:?}"));
        builder
            .add_level_resettable_sample_hold(
                instance,
                value,
                sample,
                reset,
                LogicLevel::Low,
                reset_to,
                DiagnosticMeta::default(),
            )
            .unwrap_or_else(|failure| panic!("sample hold must author: {failure:?}"));
        compile(builder)
    };
    let compiled = hold(LogicLevel::Low);
    let machine = compiled.spawn(policy());
    let replacement = hold(LogicLevel::High)
        .graph()
        .module_instances()
        .iter()
        .find(|module| module.key() == instance)
        .unwrap_or_else(|| panic!("replacement sample hold must be retained"))
        .clone();
    let report = machine.prepare_patch(
        machine
            .patch()
            .replace_module_instance(instance, replacement, ModuleMigrationDirective::Standard)
            .unwrap_or_else(|failure| panic!("reset_to replacement must build: {failure:?}"))
            .finish(),
    );
    let prepared = report
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("reset_to change must prepare: {:?}", codes(&report)));
    let module = prepared
        .static_plan()
        .modules()
        .iter()
        .find(|module| module.source() == Some(instance))
        .unwrap_or_else(|| panic!("sample hold must be classified"));
    let role = |name: &str| {
        module
            .internals()
            .iter()
            .find(|role| role.role() == name)
            .unwrap_or_else(|| panic!("sample hold must expose {name}"))
    };
    let held = role("held_state");
    assert_eq!(held.continuity(), Continuity::Preserved);
    assert_eq!(
        held.state(),
        Some(&StateCompatibility::Conditional {
            fact: "reset_to",
            when_clear: ConditionalArm::Preserve,
            when_set: ConditionalArm::Migrate,
        })
    );
    assert_eq!(role("reset_edge").continuity(), Continuity::Preserved);
    assert_eq!(
        role("reset_edge").state(),
        Some(&StateCompatibility::Preserve)
    );
    assert_eq!(
        role("reset_value_constant").continuity(),
        Continuity::ReplacedInPlace
    );
    assert!(
        prepared
            .static_plan()
            .potential_losses()
            .iter()
            .all(|loss| loss.fact() != "reset_to")
    );

    let held_key = compiled
        .graph()
        .module_instances()
        .iter()
        .find(|module| module.key() == instance)
        .and_then(|module| module.module().standard_declaration())
        .and_then(|declaration| {
            declaration.internal_roles().find(|role| {
                role.category() == StandardInternalCategory::Node && role.role() == "held_state"
            })
        })
        .map(|role| NodeKey::from_u128(role.key()))
        .unwrap_or_else(|| panic!("held state must expose a node key"));
    let same = compiled
        .graph()
        .module_instances()
        .iter()
        .find(|module| module.key() == instance)
        .unwrap_or_else(|| panic!("sample hold must be retained"))
        .clone();
    let reset = machine.prepare_patch(
        machine
            .patch()
            .replace_module_instance(
                instance,
                same,
                ModuleMigrationDirective::Explicit {
                    node_overrides: vec![ModuleNodeMigrationDirective::new(
                        held_key,
                        NodeMigrationDirective::Reset,
                    )],
                    internal_reassociations: Vec::new(),
                },
            )
            .unwrap_or_else(|failure| panic!("explicit reset must build: {failure:?}"))
            .finish(),
    );
    let reset = reset
        .artifact()
        .cloned()
        .unwrap_or_else(|| panic!("explicit held reset must prepare: {:?}", codes(&reset)));
    let reset_module = reset
        .static_plan()
        .modules()
        .iter()
        .find(|module| module.source() == Some(instance))
        .unwrap_or_else(|| panic!("reset sample hold must be classified"));
    let reset_role = |name: &str| {
        reset_module
            .internals()
            .iter()
            .find(|role| role.role() == name)
            .unwrap_or_else(|| panic!("reset sample hold must expose {name}"))
    };
    assert_eq!(
        reset_role("held_state").state(),
        Some(&StateCompatibility::Reset)
    );
    assert_eq!(
        reset_role("reset_edge").state(),
        Some(&StateCompatibility::Preserve)
    );
    assert!(
        reset.static_plan().potential_losses().iter().any(|loss| {
            loss.class() == LossClass::Unavoidable && loss.fact() == "stored_level"
        })
    );
}

#[test]
fn incompatible_or_cross_kind_directive_yields_no_artifact() {
    let (compiled, _, _) = not_gate();
    let machine = compiled.spawn(policy());
    let key = NodeKey::from_u128(2);
    let node = compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == key)
        .unwrap_or_else(|| panic!("Not must be retained"))
        .clone();
    let incompatible = machine.prepare_patch(
        machine
            .patch()
            .replace_node(key, node, NodeMigrationDirective::TransferStoredLevel)
            .unwrap_or_else(|failure| panic!("incompatible replacement must build: {failure:?}"))
            .finish(),
    );
    assert!(incompatible.artifact().is_none());
    assert!(codes(&incompatible).contains(&"reconfiguration.incompatible_migration_directive"));

    let mut builder =
        NetworkBuilder::with_key(NetworkKey::from_u128(27), TimeDomainId::from_u128(28));
    let input = builder
        .add_level_input(ExternalInputKey::from_u128(1), DiagnosticMeta::default())
        .unwrap_or_else(|failure| panic!("level input must author: {failure:?}"));
    let temporal = NodeKey::from_u128(2);
    let delay = NonZeroSpan::from_ticks(3)
        .unwrap_or_else(|failure| panic!("delay must be nonzero: {failure}"));
    builder
        .add_inertial_delay_with_ports(
            temporal,
            InPortKey::from_u128(3),
            OutPortKey::from_u128(4),
            input,
            InertialDelayConfig::new(delay, LogicLevel::Low),
            DiagnosticMeta::default(),
        )
        .unwrap_or_else(|failure| panic!("inertial delay must author: {failure:?}"));
    let compiled = compile(builder);
    let machine = compiled.spawn(policy());
    let node = compiled
        .graph()
        .nodes()
        .iter()
        .find(|node| node.key() == temporal)
        .unwrap_or_else(|| panic!("inertial delay must be retained"))
        .clone();
    let (node_key, _, ports, meta) = node.into_parts();
    let (inputs, _, outputs, _) = ports.into_parts();
    let replacement = mossignal::authored::NodeDef::new(
        node_key,
        mossignal::authored::NodeKind::transport_delay(delay, LogicLevel::Low),
        mossignal::authored::NodePorts::with_input_roles(
            inputs,
            vec![mossignal::authored::InputPortRole::TransportDelay],
            outputs,
        ),
        meta,
    );
    let crossed = machine.prepare_patch(
        machine
            .patch()
            .replace_node(temporal, replacement, NodeMigrationDirective::Standard)
            .unwrap_or_else(|failure| panic!("cross-kind replacement must build: {failure:?}"))
            .finish(),
    );
    assert!(crossed.artifact().is_none());
    assert!(
        codes(&crossed).contains(&"reconfiguration.unsupported_cross_kind_migration"),
        "cross-kind codes: {:?}",
        codes(&crossed)
    );
}
