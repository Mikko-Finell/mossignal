//! Fixed-interface standard modules, built solely from canonical primitives.

use super::*;
use crate::authored::{
    EdgeConfig, EdgeInitialization, InputPortRole, OutputPortRole, SampleHoldConfig,
};
use crate::key::{AnyInPortKey, AnyOutPortKey};
use crate::signal::Pulse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    PulseToggle,
    LevelToggle,
    SampleHold,
}

impl Kind {
    pub(crate) fn from_ref(reference: &StandardModuleRef) -> Option<Self> {
        [Self::PulseToggle, Self::LevelToggle, Self::SampleHold]
            .into_iter()
            .find(|kind| kind.reference() == *reference)
    }
    fn reference(self) -> StandardModuleRef {
        match self {
            Self::PulseToggle => StandardModuleRef::pulse_resettable_toggle(),
            Self::LevelToggle => StandardModuleRef::level_resettable_toggle(),
            Self::SampleHold => StandardModuleRef::level_resettable_sample_hold(),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::PulseToggle => "PulseResettableToggle",
            Self::LevelToggle => "LevelResettableToggle",
            Self::SampleHold => "LevelResettableSampleHold",
        }
    }
    fn output_role(self) -> &'static str {
        if self == Self::SampleHold {
            "held"
        } else {
            "state"
        }
    }
    pub(crate) fn output(self) -> ModuleOutputKey<Level> {
        ModuleOutputKey::from_u128(derive_public_key(
            PUBLIC_KEY_DOMAIN,
            self.reference().id(),
            "output",
            "level",
            self.output_role(),
        ))
    }
    fn input(self, role: &str, kind: SignalKind) -> AnyModuleInputKey {
        let key = derive_public_key(
            PUBLIC_KEY_DOMAIN,
            self.reference().id(),
            "input",
            kind_name(kind),
            role,
        );
        match kind {
            SignalKind::Level => ModuleInputKey::<Level>::from_u128(key).into(),
            SignalKind::Pulse => ModuleInputKey::<Pulse>::from_u128(key).into(),
        }
    }
    pub(crate) fn inputs(self) -> Vec<(&'static str, AnyModuleInputKey)> {
        match self {
            Self::PulseToggle => vec![
                ("toggle", self.input("toggle", SignalKind::Pulse)),
                ("reset", self.input("reset", SignalKind::Pulse)),
            ],
            Self::LevelToggle => vec![
                ("toggle", self.input("toggle", SignalKind::Pulse)),
                ("reset", self.input("reset", SignalKind::Level)),
            ],
            Self::SampleHold => vec![
                ("value", self.input("value", SignalKind::Level)),
                ("sample", self.input("sample", SignalKind::Pulse)),
                ("reset", self.input("reset", SignalKind::Level)),
            ],
        }
    }
    pub(super) fn parameter_names(self) -> &'static [&'static str] {
        if self == Self::SampleHold {
            &["initial", "reset_to"]
        } else {
            &["initial"]
        }
    }
    pub(crate) fn descriptor<D>(self) -> StandardModuleDescriptor<D> {
        StandardModuleDescriptor {
            module_ref: self.reference(),
            introduced: StandardCatalogueVersion::one(),
            display_name: self.name(),
            documentation: match self {
                Self::PulseToggle => "Reset-dominant pulse-controlled parity state.",
                Self::LevelToggle => "Parity state held Low while reset is High.",
                Self::SampleHold => "Capture value or reset target on sample or rising reset.",
            },
            category: StandardModuleCategory::Stateful,
            availability: StandardModuleAvailability::Available,
            inputs: self
                .inputs()
                .into_iter()
                .map(|(role, key)| StandardPortSchema {
                    role,
                    kind: key.kind(),
                    variadic: false,
                    fixed_input: Some(key),
                    fixed_output: None,
                })
                .collect(),
            outputs: vec![StandardPortSchema {
                role: self.output_role(),
                kind: SignalKind::Level,
                variadic: false,
                fixed_input: None,
                fixed_output: Some(self.output().into()),
            }],
            parameters: self
                .parameter_names()
                .iter()
                .map(|name| StandardParameterSchema {
                    key: StandardParameterKey::new(*name),
                    kind: StandardParameterKind::LogicLevel,
                    required: true,
                })
                .collect(),
            marker: PhantomData,
        }
    }
}

fn kind_name(kind: SignalKind) -> &'static str {
    match kind {
        SignalKind::Level => "level",
        SignalKind::Pulse => "pulse",
    }
}

macro_rules! input_key {
    ($name:ident, $kind:ident, $role:literal, $signal:ident, $doc:literal) => {
        #[doc = $doc]
        #[must_use]
        pub fn $name() -> ModuleInputKey<$signal> {
            ModuleInputKey::from_u128(derive_public_key(
                PUBLIC_KEY_DOMAIN,
                Kind::$kind.reference().id(),
                "input",
                kind_name(<$signal as crate::signal::SignalType>::KIND),
                $role,
            ))
        }
    };
}
input_key!(
    pulse_resettable_toggle_toggle_key,
    PulseToggle,
    "toggle",
    Pulse,
    "Fixed toggle input of PulseResettableToggle."
);
input_key!(
    pulse_resettable_toggle_reset_key,
    PulseToggle,
    "reset",
    Pulse,
    "Fixed reset input of PulseResettableToggle."
);
input_key!(
    level_resettable_toggle_toggle_key,
    LevelToggle,
    "toggle",
    Pulse,
    "Fixed toggle input of LevelResettableToggle."
);
input_key!(
    level_resettable_toggle_reset_key,
    LevelToggle,
    "reset",
    Level,
    "Fixed reset input of LevelResettableToggle."
);
input_key!(
    level_resettable_sample_hold_value_key,
    SampleHold,
    "value",
    Level,
    "Fixed value input of LevelResettableSampleHold."
);
input_key!(
    level_resettable_sample_hold_sample_key,
    SampleHold,
    "sample",
    Pulse,
    "Fixed sample input of LevelResettableSampleHold."
);
input_key!(
    level_resettable_sample_hold_reset_key,
    SampleHold,
    "reset",
    Level,
    "Fixed reset input of LevelResettableSampleHold."
);
/// Fixed state output of PulseResettableToggle.
#[must_use]
pub fn pulse_resettable_toggle_state_key() -> ModuleOutputKey<Level> {
    Kind::PulseToggle.output()
}
/// Fixed state output of LevelResettableToggle.
#[must_use]
pub fn level_resettable_toggle_state_key() -> ModuleOutputKey<Level> {
    Kind::LevelToggle.output()
}
/// Fixed held output of LevelResettableSampleHold.
#[must_use]
pub fn level_resettable_sample_hold_held_key() -> ModuleOutputKey<Level> {
    Kind::SampleHold.output()
}

pub(super) fn build<D: PartialEq>(
    kind: Kind,
    request: StandardModuleRequest<D>,
) -> Report<ModuleDef<D>, D> {
    let mut diagnostics = DiagnosticSet::new();
    let mut parameters = std::collections::BTreeMap::new();
    for (key, value) in &request.parameters {
        let evidence =
            if !kind.parameter_names().contains(&key.as_str()) || parameters.contains_key(key) {
                Some(ProblemEvidence::standard_module_unexpected_parameter(
                    request.module_ref.clone(),
                    key.clone(),
                ))
            } else if let StandardParameterValue::LogicLevel(level) = value {
                parameters.insert(key.clone(), *level);
                None
            } else {
                Some(ProblemEvidence::standard_module_parameter_kind_mismatch(
                    request.module_ref.clone(),
                    key.clone(),
                    StandardParameterKind::LogicLevel,
                    value.kind(),
                ))
            };
        if let Some(evidence) = evidence {
            insert_evidence(&mut diagnostics, evidence);
        }
    }
    for name in kind.parameter_names() {
        let key = StandardParameterKey::new(*name);
        if !parameters.contains_key(&key) {
            insert_evidence(
                &mut diagnostics,
                ProblemEvidence::standard_module_missing_parameter(request.module_ref.clone(), key),
            );
        }
    }
    if !request.variadic_inputs.is_empty() {
        insert_evidence(
            &mut diagnostics,
            ProblemEvidence::standard_module_interface_mismatch(
                request.module_ref.clone(),
                request.variadic_inputs,
            ),
        );
    }
    if diagnostics.has_severity(Severity::Error) {
        return Report::new(None, diagnostics);
    }
    let initial = parameters[&StandardParameterKey::new("initial")];
    let reset_to = parameters
        .get(&StandardParameterKey::new("reset_to"))
        .copied();
    let (unchecked, roles) = expand(kind, initial, reset_to);
    let mut seen = BTreeSet::new();
    for role in &roles {
        if !seen.insert((role.category, role.key)) {
            insert_evidence(
                &mut diagnostics,
                ProblemEvidence::standard_module_internal_key_collision(
                    request.module_ref.clone(),
                    role.key,
                ),
            );
        }
    }
    if diagnostics.has_severity(Severity::Error) {
        return Report::new(None, diagnostics);
    }
    let (module, findings) = unchecked.validate().into_parts();
    for finding in findings {
        diagnostics.insert(finding);
    }
    let Some(module) = module else {
        return Report::new(None, diagnostics);
    };
    let fingerprint =
        expansion_fingerprint(module.definition(), &request.module_ref, None, &[], &roles);
    let declaration = StandardModuleDeclaration {
        module_ref: request.module_ref,
        parameters: parameters
            .into_iter()
            .map(|(key, value)| StandardParameterAssignment {
                key,
                value: StandardParameterValue::LogicLevel(value),
            })
            .collect(),
        variadic_inputs: Vec::new(),
        expansion_fingerprint: fingerprint,
        internal_roles: roles,
    };
    Report::new(Some(module.with_standard_origin(declaration)), diagnostics)
}

#[derive(Clone, Copy)]
enum Source {
    Public(AnyModuleInputKey),
    Node(AnyOutPortKey),
}
impl Source {
    fn kind(self) -> SignalKind {
        match self {
            Self::Public(key) => key.kind(),
            Self::Node(key) => key.kind(),
        }
    }
    fn endpoint(self) -> ConnectionEndpoint {
        match self {
            Self::Public(key) => ConnectionEndpoint::module_input(key),
            Self::Node(key) => ConnectionEndpoint::node_output(key),
        }
    }
}

struct FixedExpansion<D> {
    inner: Expansion<D>,
}
impl<D> FixedExpansion<D> {
    fn key(&self, category: &str, role: &str, kind: SignalKind) -> u128 {
        derive_internal_key(
            INTERNAL_KEY_DOMAIN,
            self.inner.module_ref.id(),
            category,
            kind_name(kind),
            role,
            None,
        )
    }
    fn source_role(&self, source: Source) -> String {
        match source {
            Source::Node(key) => required(self.inner.roles.iter().find(|role| {
                role.category == StandardInternalCategory::OutputPort
                    && role.key
                        == match key {
                            AnyOutPortKey::Level(key) => key.as_u128(),
                            AnyOutPortKey::Pulse(key) => key.as_u128(),
                        }
            }))
            .role
            .clone(),
            Source::Public(key) => {
                let kind = required(Kind::from_ref(&self.inner.module_ref));
                required(
                    kind.inputs()
                        .into_iter()
                        .find(|(_, candidate)| *candidate == key),
                )
                .0
                .to_owned()
            }
        }
    }
    fn node(
        &mut self,
        role: &str,
        kind: NodeKind<D>,
        inputs: &[(InputPortRole, Source)],
        output_kind: SignalKind,
    ) -> Source {
        let node = NodeKey::from_u128(self.key("node", role, output_kind));
        self.inner
            .record(StandardInternalCategory::Node, role, node.as_u128(), None);
        let mut ports = Vec::new();
        for (port_role, source) in inputs {
            let source_role = self.source_role(*source);
            let semantic_role = match port_role {
                InputPortRole::Input => "input",
                InputPortRole::Toggle => "toggle",
                InputPortRole::Pulses => "pulses",
                InputPortRole::Enable => "enable",
                InputPortRole::Value => "value",
                InputPortRole::Sample => "sample",
                InputPortRole::Selector => "selector",
                InputPortRole::WhenLow => "when_low",
                InputPortRole::WhenHigh => "when_high",
                _ => panic!(
                    "canonical stateful expansion uses only its declared primitive port roles"
                ),
            };
            let role = if crate::node_schema::node_schema(&kind).is_variadic() {
                format!("{role}.{semantic_role}.{source_role}")
            } else {
                format!("{role}.{semantic_role}")
            };
            let payload = self.key("input_port", &role, source.kind());
            let port: AnyInPortKey = match source.kind() {
                SignalKind::Level => InPortKey::<Level>::from_u128(payload).into(),
                SignalKind::Pulse => InPortKey::<Pulse>::from_u128(payload).into(),
            };
            self.inner
                .record(StandardInternalCategory::InputPort, &role, payload, None);
            ports.push((port, *port_role));
            match source {
                Source::Public(key) => self.inner.mappings.push(ModuleInterfaceMapping::input(
                    *key,
                    ConnectionEndpoint::node_input(port),
                )),
                Source::Node(output) => {
                    let incidence = format!("{source_role}->{role}");
                    let key =
                        ConnectionKey::from_u128(self.key("connection", &incidence, source.kind()));
                    self.inner.record(
                        StandardInternalCategory::Connection,
                        &incidence,
                        key.as_u128(),
                        None,
                    );
                    self.inner.connections.push(ConnectionDef::new(
                        key,
                        ConnectionEndpoint::node_output(*output),
                        ConnectionEndpoint::node_input(port),
                        DiagnosticMeta::default(),
                    ));
                }
            }
        }
        let out_role = format!("{role}.output");
        let payload = self.key("output_port", &out_role, output_kind);
        let output: AnyOutPortKey = match output_kind {
            SignalKind::Level => OutPortKey::<Level>::from_u128(payload).into(),
            SignalKind::Pulse => OutPortKey::<Pulse>::from_u128(payload).into(),
        };
        self.inner.record(
            StandardInternalCategory::OutputPort,
            &out_role,
            payload,
            None,
        );
        self.inner.nodes.push(NodeDef::new(
            node,
            kind,
            NodePorts::with_roles(
                ports.iter().map(|(key, _)| *key).collect(),
                ports.iter().map(|(_, role)| *role).collect(),
                vec![output],
                vec![OutputPortRole::Output],
            ),
            DiagnosticMeta::default(),
        ));
        Source::Node(output)
    }
}

pub(super) fn expand<D>(
    kind: Kind,
    initial: LogicLevel,
    reset_to: Option<LogicLevel>,
) -> (UncheckedModule<D>, Vec<StandardInternalRole>) {
    use InputPortRole as R;
    use SignalKind::{Level as L, Pulse as P};
    let mut e = FixedExpansion {
        inner: Expansion::new(&kind.reference()),
    };
    let public = |role, signal| Source::Public(kind.input(role, signal));
    // SPEC: docs/specs/contracts/pulse-resettable-toggle.yaml "canonical-expansion-and-aggregate"
    // SPEC: docs/specs/contracts/level-resettable-toggle.yaml "canonical-expansion"
    // SPEC: docs/specs/contracts/level-resettable-sample-hold.yaml "canonical-expansion"
    // These exact role graphs are observable identity, not replaceable Boolean optimizations.
    let result = match kind {
        Kind::PulseToggle | Kind::LevelToggle => {
            let reset = public("reset", if kind == Kind::PulseToggle { P } else { L });
            let toggle = public("toggle", P);
            let (accepted, edge, low) = if kind == Kind::LevelToggle {
                let inverse = e.node("reset_inverter", NodeKind::not(), &[(R::Input, reset)], L);
                let accepted = e.node(
                    "accepted_toggle",
                    NodeKind::pulse_gate(),
                    &[(R::Pulses, toggle), (R::Enable, inverse)],
                    P,
                );
                let low = e.node("low_constant", NodeKind::constant(LogicLevel::Low), &[], L);
                let edge = e.node(
                    "reset_edge",
                    NodeKind::rising_edge(EdgeConfig::new(EdgeInitialization::Assume(
                        LogicLevel::Low,
                    ))),
                    &[(R::Input, reset)],
                    P,
                );
                (accepted, edge, Some(low))
            } else {
                (toggle, reset, None)
            };
            let state = e.node(
                "toggle_state",
                NodeKind::toggle(initial),
                &[(R::Toggle, accepted)],
                L,
            );
            let baseline = e.node(
                "reset_baseline",
                NodeKind::sample_hold(SampleHoldConfig::new(LogicLevel::Low)),
                &[(R::Value, state), (R::Sample, edge)],
                L,
            );
            let relative = e.node(
                "relative_state",
                NodeKind::parity(),
                &[(R::Input, state), (R::Input, baseline)],
                L,
            );
            if let Some(low) = low {
                e.node(
                    "reset_select",
                    NodeKind::select(),
                    &[
                        (R::Selector, reset),
                        (R::WhenLow, relative),
                        (R::WhenHigh, low),
                    ],
                    L,
                )
            } else {
                relative
            }
        }
        Kind::SampleHold => {
            let reset = public("reset", L);
            let target = match reset_to {
                Some(value) => value,
                None => panic!("validated resettable sample-hold parameters require reset_to"),
            };
            let constant = e.node("reset_value_constant", NodeKind::constant(target), &[], L);
            let value = e.node(
                "capture_value",
                NodeKind::select(),
                &[
                    (R::Selector, reset),
                    (R::WhenLow, public("value", L)),
                    (R::WhenHigh, constant),
                ],
                L,
            );
            let edge = e.node(
                "reset_edge",
                NodeKind::rising_edge(EdgeConfig::new(EdgeInitialization::Assume(LogicLevel::Low))),
                &[(R::Input, reset)],
                P,
            );
            let pulses = e.node(
                "capture_pulses",
                NodeKind::merge(),
                &[(R::Input, public("sample", P)), (R::Input, edge)],
                P,
            );
            e.node(
                "held_state",
                NodeKind::sample_hold(SampleHoldConfig::new(initial)),
                &[(R::Value, value), (R::Sample, pulses)],
                L,
            )
        }
    };
    let export_role = format!("{}<-{}", kind.output_role(), e.source_role(result));
    let export = e.key("export", &export_role, L);
    e.inner
        .record(StandardInternalCategory::Export, &export_role, export, None);
    e.inner.mappings.push(ModuleInterfaceMapping::output(
        kind.output().into(),
        result.endpoint(),
    ));
    e.inner
        .roles
        .sort_by(|a, b| (a.category, &a.role, a.key).cmp(&(b.category, &b.role, b.key)));
    (
        UncheckedModule::new_user(
            DiagnosticMeta::default(),
            kind.inputs()
                .into_iter()
                .map(|(_, key)| ModuleInputDef::new(key, DiagnosticMeta::default()))
                .collect(),
            vec![ModuleOutputDef::new(
                kind.output().into(),
                DiagnosticMeta::default(),
            )],
            e.inner.mappings,
            e.inner.nodes,
            e.inner.connections,
        ),
        e.inner.roles,
    )
}

use crate::compile::FullEvaluation;
use crate::signal::PulseCount;
use crate::time::Time;
use crate::{CauseRef, CompiledNetwork, ProvenanceView, QualifiedModuleRef, QualifiedNodeRef};
use std::collections::BTreeMap;

/// Reset facts from a completed reaction, not persistent pulse activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetObservation {
    /// Simultaneous reset pulse multiplicity.
    Pulse(PulseCount),
    /// Settled level and whether it rose in that reaction.
    Level { level: LogicLevel, rose: bool },
}
impl ResetObservation {
    /// Whether reset controls this reaction's result.
    #[must_use]
    pub fn asserted(self) -> bool {
        match self {
            Self::Pulse(n) => n.is_positive(),
            Self::Level { level, .. } => level.is_high(),
        }
    }
}

/// Historical reason a sample-hold captured in its latest reaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    None,
    Sample,
    Reset,
    Both,
}

/// Structured public-law explanation of the last completed reaction.
/// Counts here are explicitly historical and never current pulse signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatefulStandardReaction {
    /// Reset-dominant parity, with public and accepted/suppressed multiplicities.
    /// Suppression describes the aggregate decision; PulseResettableToggle's
    /// internal toggle still consumes the batch before reset samples it.
    Toggle {
        previous: LogicLevel,
        toggle_count: PulseCount,
        reset: ResetObservation,
        accepted: PulseCount,
        suppressed: PulseCount,
        result: LogicLevel,
    },
    /// The selected value and capture decision of resettable sample storage.
    SampleHold {
        previous: LogicLevel,
        value: LogicLevel,
        sample_count: PulseCount,
        reset: LogicLevel,
        reset_rose: bool,
        capture: CaptureKind,
        selected: LogicLevel,
        result: LogicLevel,
    },
}

/// A structured explanation for why a requested change or value did not occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatefulWhyNot {
    /// An asserted reset controls the result.
    ResetDominated,
    /// No toggle pulses arrived.
    NoToggle,
    /// A positive even count leaves parity unchanged.
    EvenToggleParity,
    /// Accepted parity resulted in Low.
    ResultingParityLow,
    /// No sample or reset edge requested a capture.
    NoCapture,
    /// Sampling established the same value already stored.
    CapturedSameValue,
    /// Reset selected reset_to instead of the public value input.
    ResetSelectedTarget,
}
impl StatefulStandardReaction {
    /// Explains an unchanged output, or returns None when the output changed.
    #[must_use]
    pub fn why_not_change(self) -> Option<StatefulWhyNot> {
        match self {
            Self::Toggle {
                previous, result, ..
            }
            | Self::SampleHold {
                previous, result, ..
            } if previous != result => None,
            Self::Toggle { reset, .. } if reset.asserted() => Some(StatefulWhyNot::ResetDominated),
            Self::Toggle { toggle_count, .. } if toggle_count.is_zero() => {
                Some(StatefulWhyNot::NoToggle)
            }
            Self::Toggle { .. } => Some(StatefulWhyNot::EvenToggleParity),
            Self::SampleHold {
                capture: CaptureKind::None,
                ..
            } => Some(StatefulWhyNot::NoCapture),
            Self::SampleHold { .. } => Some(StatefulWhyNot::CapturedSameValue),
        }
    }
    /// Explains a Low toggle result; inapplicable to sample storage or High output.
    #[must_use]
    pub fn why_not_high(self) -> Option<StatefulWhyNot> {
        match self {
            Self::Toggle {
                result: LogicLevel::Low,
                reset,
                ..
            } => Some(if reset.asserted() {
                StatefulWhyNot::ResetDominated
            } else {
                StatefulWhyNot::ResultingParityLow
            }),
            _ => None,
        }
    }
    /// Explains reset selecting its configured target instead of the value input.
    #[must_use]
    pub fn why_not_sample_value(self) -> Option<StatefulWhyNot> {
        match self {
            Self::SampleHold {
                reset: LogicLevel::High,
                ..
            } => Some(StatefulWhyNot::ResetSelectedTarget),
            _ => None,
        }
    }
}

/// Owned aggregate and historical observation of one stateful standard module.
/// The expanded primitive state remains available through `ModuleInspection::nodes`.
/// All causes resolve through this value's retained provenance, even after advancement.
pub struct StatefulStandardInspection<D> {
    /// Qualified instance identity, including nested containment.
    pub module: QualifiedModuleRef,
    /// Exact standard descriptor.
    pub module_ref: StandardModuleRef,
    /// Time of the current state and last completed reaction.
    pub at: Time<D>,
    /// Configured fresh state.
    pub initial: LogicLevel,
    /// Configured reset capture target, for sample storage only.
    pub reset_to: Option<LogicLevel>,
    /// Current aggregate state.
    pub state: LogicLevel,
    /// Internal toggle stored level, for toggle modules only.
    pub toggle_state: Option<LogicLevel>,
    /// Internal sampled reset baseline, for toggle modules only.
    pub reset_baseline: Option<LogicLevel>,
    /// Committed reset-edge observation, for level-controlled reset only.
    pub remembered_reset: Option<LogicLevel>,
    /// Most recent reset pulse or reset-level transition cause, when one has occurred.
    pub latest_reset_cause: Option<CauseRef>,
    /// Most recent nonzero accepted toggle batch, including even batches.
    pub latest_accepted_toggle_cause: Option<CauseRef>,
    /// Most recent sample-hold capture cause, when one has occurred.
    pub latest_capture_cause: Option<CauseRef>,
    /// Historical public facts and public-law explanation for the last reaction.
    pub last_reaction: StatefulStandardReaction,
    /// Last-reaction public input support, including suppressed inputs.
    pub public_causes: Vec<(AnyModuleInputKey, CauseRef)>,
    /// Canonical qualified primitive drill-down, sorted by qualified identity.
    pub internal_causes: Vec<(QualifiedNodeRef, CauseRef)>,
    /// Provenance retaining all causes in this owned view.
    pub provenance: ProvenanceView<D>,
}

#[derive(Clone)]
pub(crate) struct StandardHistory {
    state: LogicLevel,
    toggle_state: Option<LogicLevel>,
    reset_baseline: Option<LogicLevel>,
    remembered_reset: Option<LogicLevel>,
    latest_reset: Option<CauseRef>,
    latest_toggle: Option<CauseRef>,
    latest_capture: Option<CauseRef>,
    reaction: StatefulStandardReaction,
}

impl StandardHistory {
    pub(crate) fn retained_causes(&self) -> impl Iterator<Item = CauseRef> {
        [self.latest_reset, self.latest_toggle, self.latest_capture]
            .into_iter()
            .flatten()
    }

    pub(crate) fn translate_causes(&mut self, translate: impl Fn(CauseRef) -> CauseRef) {
        self.latest_reset = self.latest_reset.map(&translate);
        self.latest_toggle = self.latest_toggle.map(&translate);
        self.latest_capture = self.latest_capture.map(&translate);
    }
}

fn required<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => panic!(
            "validated canonical standard module must retain every required role, parameter, state slot and reaction value"
        ),
    }
}

pub(crate) fn observe_reaction<D>(
    compiled: &CompiledNetwork<D>,
    evaluation: &FullEvaluation,
    causes: &[CauseRef],
    previous: &BTreeMap<QualifiedModuleRef, StandardHistory>,
    rebase: impl Fn(CauseRef) -> CauseRef,
) -> BTreeMap<QualifiedModuleRef, StandardHistory> {
    compiled
        .standard_modules()
        .filter_map(|(path, definition)| {
            let declaration = definition.standard_declaration()?;
            let kind = Kind::from_ref(declaration.module_ref())?;
            let nodes: BTreeMap<_, _> = compiled
                .qualified_nodes_under(path)
                .map(|(qualified, flat)| (qualified.node(), flat))
                .collect();
            let role_node = |role| {
                let local = required(declaration.internal_roles().find(|item| {
                    item.category() == StandardInternalCategory::Node && item.role() == role
                }));
                required(nodes.get(&NodeKey::from_u128(local.key())).copied())
            };
            let input_op = |role, signal| {
                required(compiled.module_input_operation(path, kind.input(role, signal)))
            };
            let level =
                |role| required(evaluation.operation_levels[input_op(role, SignalKind::Level)]);
            let pulse =
                |role| required(evaluation.operation_pulses[input_op(role, SignalKind::Pulse)]);
            let cause = |role, signal| causes[input_op(role, signal)];
            let stored = |role| {
                let node = role_node(role);
                let (slot, _) = required(
                    compiled
                        .toggle_state_slot(node)
                        .or_else(|| compiled.sample_hold_state_slot(node)),
                );
                (
                    evaluation.previous_stored_levels[slot.value()],
                    evaluation.proposed_stored_levels[slot.value()],
                )
            };
            let output_op = required(compiled.module_output_operation(path, kind.output().into()));
            let state = required(evaluation.operation_levels[output_op]);
            let remembered_reset = (kind != Kind::PulseToggle).then(|| level("reset"));
            let reset_changed = if kind != Kind::PulseToggle {
                let (slot, _, _) = required(compiled.edge_state_slot(role_node("reset_edge")));
                debug_assert_eq!(
                    evaluation.proposed_edge_observations[slot.value()],
                    crate::EdgeObservation::Established(level("reset")),
                    "standard reset edge must remember settled reset"
                );
                evaluation.previous_edge_observations[slot.value()]
                    != crate::EdgeObservation::Established(level("reset"))
            } else {
                false
            };
            let rose = reset_changed && level("reset").is_high();
            let old = previous.get(path);
            let mut history = StandardHistory {
                state,
                toggle_state: None,
                reset_baseline: None,
                remembered_reset,
                latest_reset: old.and_then(|h| h.latest_reset).map(&rebase),
                latest_toggle: old.and_then(|h| h.latest_toggle).map(&rebase),
                latest_capture: old.and_then(|h| h.latest_capture).map(&rebase),
                reaction: StatefulStandardReaction::SampleHold {
                    previous: state,
                    value: state,
                    sample_count: PulseCount::ZERO,
                    reset: LogicLevel::Low,
                    reset_rose: false,
                    capture: CaptureKind::None,
                    selected: state,
                    result: state,
                },
            };
            if kind == Kind::SampleHold {
                let (previous, held) = stored("held_state");
                debug_assert_eq!(held, state, "standard aggregate must equal held state");
                let sample = pulse("sample");
                let capture = match (sample.is_positive(), rose) {
                    (false, false) => CaptureKind::None,
                    (true, false) => CaptureKind::Sample,
                    (false, true) => CaptureKind::Reset,
                    (true, true) => CaptureKind::Both,
                };
                let selected = required(
                    evaluation.operation_levels
                        [required(compiled.node_operation(role_node("capture_value")))],
                );
                history.reaction = StatefulStandardReaction::SampleHold {
                    previous,
                    value: level("value"),
                    sample_count: sample,
                    reset: level("reset"),
                    reset_rose: rose,
                    capture,
                    selected,
                    result: state,
                };
                if capture != CaptureKind::None {
                    history.latest_capture =
                        Some(causes[required(compiled.node_operation(role_node("held_state")))]);
                }
            } else {
                let (a, after_a) = stored("toggle_state");
                let (b, after_b) = stored("reset_baseline");
                let xor = |a, b| {
                    if a != b {
                        LogicLevel::High
                    } else {
                        LogicLevel::Low
                    }
                };
                debug_assert_eq!(
                    state,
                    xor(after_a, after_b),
                    "standard aggregate must derive from toggle and baseline"
                );
                if remembered_reset == Some(LogicLevel::High) {
                    debug_assert_eq!(
                        after_a, after_b,
                        "held reset must establish equal internal state"
                    );
                }
                history.toggle_state = Some(after_a);
                history.reset_baseline = Some(after_b);
                let count = pulse("toggle");
                let reset = if kind == Kind::PulseToggle {
                    ResetObservation::Pulse(pulse("reset"))
                } else {
                    ResetObservation::Level {
                        level: level("reset"),
                        rose,
                    }
                };
                let accepted = if reset.asserted() {
                    PulseCount::ZERO
                } else {
                    count
                };
                let suppressed = if reset.asserted() {
                    count
                } else {
                    PulseCount::ZERO
                };
                history.reaction = StatefulStandardReaction::Toggle {
                    previous: xor(a, b),
                    toggle_count: count,
                    reset,
                    accepted,
                    suppressed,
                    result: state,
                };
                if accepted.is_positive() {
                    history.latest_toggle = Some(cause("toggle", SignalKind::Pulse));
                }
            }
            if reset_changed || (kind == Kind::PulseToggle && pulse("reset").is_positive()) {
                history.latest_reset = Some(if kind == Kind::PulseToggle {
                    cause("reset", SignalKind::Pulse)
                } else {
                    causes[required(compiled.node_operation(role_node("reset_edge")))]
                });
            }
            // SPEC: docs/specs/standard_module_catalogue_spec.md §30 "Cross-internal state invariants"
            // Exactly the declared state owners must exist; additional state is not harmless.
            #[cfg(debug_assertions)]
            {
                let actual: BTreeSet<_> = definition
                    .graph()
                    .nodes()
                    .iter()
                    .filter(|node| {
                        crate::node_schema::node_schema(node.kind())
                            .state_family()
                            .is_some()
                    })
                    .map(|node| nodes[&node.key()])
                    .collect();
                let expected: BTreeSet<_> = match kind {
                    Kind::PulseToggle => {
                        vec![role_node("toggle_state"), role_node("reset_baseline")]
                    }
                    Kind::LevelToggle => vec![
                        role_node("toggle_state"),
                        role_node("reset_baseline"),
                        role_node("reset_edge"),
                    ],
                    Kind::SampleHold => vec![role_node("held_state"), role_node("reset_edge")],
                }
                .into_iter()
                .collect();
                debug_assert_eq!(
                    actual, expected,
                    "canonical module must have exactly its declared state owners"
                );
            }
            Some((path.clone(), history))
        })
        .collect()
}

pub(crate) fn inspect<D>(
    machine: &crate::Machine<D>,
    path: &QualifiedModuleRef,
) -> Option<StatefulStandardInspection<D>> {
    let history = machine.store.standard_history.get(path)?;
    let declaration = machine.compiled.module(path)?.standard_declaration()?;
    let kind = Kind::from_ref(declaration.module_ref())?;
    let parameter = |name| {
        declaration.parameters().find_map(|p| match p.value() {
            StandardParameterValue::LogicLevel(value) if p.key().as_str() == name => Some(*value),
            _ => None,
        })
    };
    let public_causes: Vec<_> = kind
        .inputs()
        .into_iter()
        .map(|(_, key)| {
            (
                key,
                machine.store.operation_causes
                    [required(machine.compiled.module_input_operation(path, key))],
            )
        })
        .collect();
    let internal_causes: Vec<_> = machine
        .compiled
        .qualified_nodes_under(path)
        .map(|(qualified, flat)| {
            (
                qualified.clone(),
                machine.store.operation_causes[required(machine.compiled.node_operation(flat))],
            )
        })
        .collect();
    let roots: Vec<_> = public_causes
        .iter()
        .map(|(_, cause)| *cause)
        .chain(internal_causes.iter().map(|(_, cause)| *cause))
        .chain(history.latest_reset)
        .chain(history.latest_toggle)
        .chain(history.latest_capture)
        .collect();
    Some(StatefulStandardInspection {
        module: path.clone(),
        module_ref: declaration.module_ref().clone(),
        at: machine.now()?,
        initial: parameter("initial")?,
        reset_to: parameter("reset_to"),
        state: history.state,
        toggle_state: history.toggle_state,
        reset_baseline: history.reset_baseline,
        remembered_reset: history.remembered_reset,
        latest_reset_cause: history.latest_reset,
        latest_accepted_toggle_cause: history.latest_toggle,
        latest_capture_cause: history.latest_capture,
        last_reaction: history.reaction,
        public_causes,
        internal_causes,
        provenance: machine.store.provenance.as_ref()?.owned_roots(&roots),
    })
}
