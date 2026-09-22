//! Private declarative schema for the closed built-in node family.

use crate::authored::{InputPortRole, NodeKind, OutputPortRole};
use crate::signal::SignalKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SemanticNodeKind {
    Constant,
    Not,
    All,
    Any,
    Parity,
    AtLeast,
    Select,
    Merge,
    Coalesce,
    Zip,
    PulseGate,
    PulseSelect,
    PulseRoute,
    RisingEdge,
    FallingEdge,
    AnyEdge,
    Toggle,
    PulseSetResetLatch,
    LevelSetResetLatch,
    SampleHold,
    PulseDelay,
    TransportDelay,
    InertialDelay,
    Periodic,
}

impl SemanticNodeKind {
    pub(crate) const fn identity_tag(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Not => "not",
            Self::All => "all",
            Self::Any => "any",
            Self::Parity => "parity",
            Self::AtLeast => "at_least",
            Self::Select => "select",
            Self::Merge => "merge",
            Self::Coalesce => "coalesce",
            Self::Zip => "zip",
            Self::PulseGate => "pulse_gate",
            Self::PulseSelect => "pulse_select",
            Self::PulseRoute => "pulse_route",
            Self::RisingEdge => "rising_edge",
            Self::FallingEdge => "falling_edge",
            Self::AnyEdge => "any_edge",
            Self::Toggle => "toggle",
            Self::PulseSetResetLatch => "pulse_set_reset_latch",
            Self::LevelSetResetLatch => "level_set_reset_latch",
            Self::SampleHold => "sample_hold",
            Self::PulseDelay => "pulse_delay",
            Self::TransportDelay => "transport_delay",
            Self::InertialDelay => "inertial_delay",
            Self::Periodic => "periodic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateFamily {
    EdgeObservation,
    StoredLevel,
    TransportLevel,
    PeriodicEnable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemporalFamily {
    PendingPulseGroup,
    PendingTransportTransition,
    InertialCandidate,
    PeriodicBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentOutputDependency {
    None,
    AllOutputs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InputPortSchema {
    role: InputPortRole,
    kind: SignalKind,
    current_output_dependency: CurrentOutputDependency,
}

impl InputPortSchema {
    const fn current(role: InputPortRole, kind: SignalKind) -> Self {
        Self {
            role,
            kind,
            current_output_dependency: CurrentOutputDependency::AllOutputs,
        }
    }

    const fn future_only(role: InputPortRole, kind: SignalKind) -> Self {
        Self {
            role,
            kind,
            current_output_dependency: CurrentOutputDependency::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutputPortSchema {
    role: OutputPortRole,
    kind: SignalKind,
}

impl OutputPortSchema {
    const fn new(role: OutputPortRole, kind: SignalKind) -> Self {
        Self { role, kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputShape {
    Fixed(&'static [InputPortSchema]),
    Variadic {
        port: InputPortSchema,
        minimum: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeSchema {
    semantic_kind: SemanticNodeKind,
    inputs: InputShape,
    outputs: &'static [OutputPortSchema],
    state_family: Option<StateFamily>,
    temporal_family: Option<TemporalFamily>,
}

impl NodeSchema {
    pub(crate) const fn semantic_kind(self) -> SemanticNodeKind {
        self.semantic_kind
    }

    pub(crate) const fn expected_input_count(self) -> Option<usize> {
        match self.inputs {
            InputShape::Fixed(inputs) => Some(inputs.len()),
            InputShape::Variadic { .. } => None,
        }
    }

    pub(crate) const fn minimum_input_count(self) -> usize {
        match self.inputs {
            InputShape::Fixed(inputs) => inputs.len(),
            InputShape::Variadic { minimum, .. } => minimum,
        }
    }

    pub(crate) const fn accepts_input_count(self, count: usize) -> bool {
        match self.inputs {
            InputShape::Fixed(inputs) => count == inputs.len(),
            InputShape::Variadic { minimum, .. } => count >= minimum,
        }
    }

    pub(crate) const fn is_variadic(self) -> bool {
        matches!(self.inputs, InputShape::Variadic { .. })
    }

    pub(crate) const fn expected_output_count(self) -> usize {
        self.outputs.len()
    }

    pub(crate) fn required_input_roles(self, actual_count: usize) -> Vec<InputPortRole> {
        match self.inputs {
            InputShape::Fixed(inputs) => inputs.iter().map(|input| input.role).collect(),
            InputShape::Variadic { port, .. } => vec![port.role; actual_count],
        }
    }

    pub(crate) fn required_output_roles(self, actual_count: usize) -> Vec<OutputPortRole> {
        match self.outputs {
            [output] if output.role == OutputPortRole::Output => {
                vec![OutputPortRole::Output; actual_count]
            }
            outputs => outputs.iter().map(|output| output.role).collect(),
        }
    }

    pub(crate) fn input_kind(self, role: InputPortRole) -> Option<SignalKind> {
        match self.inputs {
            InputShape::Fixed(inputs) => find_input_kind(inputs, role),
            InputShape::Variadic { port, .. } => {
                if port.role == role {
                    Some(port.kind)
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn output_kind(self, role: OutputPortRole) -> Option<SignalKind> {
        find_output_kind(self.outputs, role)
    }

    pub(crate) fn accepts_output_kind(self, kind: SignalKind) -> bool {
        self.outputs.iter().any(|output| output.kind == kind)
    }

    pub(crate) fn input_affects_any_current_output(
        self,
        input_role: InputPortRole,
        output_roles: &[OutputPortRole],
    ) -> bool {
        let dependency = match self.inputs {
            InputShape::Fixed(inputs) => find_input_dependency(inputs, input_role),
            InputShape::Variadic { port, .. } if port.role == input_role => {
                Some(port.current_output_dependency)
            }
            InputShape::Variadic { .. } => None,
        };
        dependency == Some(CurrentOutputDependency::AllOutputs)
            && output_roles
                .iter()
                .any(|role| self.output_kind(*role).is_some())
    }

    pub(crate) const fn state_family(self) -> Option<StateFamily> {
        self.state_family
    }

    pub(crate) const fn temporal_family(self) -> Option<TemporalFamily> {
        self.temporal_family
    }
}

fn find_input_kind(inputs: &[InputPortSchema], role: InputPortRole) -> Option<SignalKind> {
    let mut index = 0;
    while index < inputs.len() {
        if inputs[index].role == role {
            return Some(inputs[index].kind);
        }
        index += 1;
    }
    None
}

fn find_output_kind(outputs: &[OutputPortSchema], role: OutputPortRole) -> Option<SignalKind> {
    let mut index = 0;
    while index < outputs.len() {
        if outputs[index].role == role {
            return Some(outputs[index].kind);
        }
        index += 1;
    }
    None
}

fn find_input_dependency(
    inputs: &[InputPortSchema],
    role: InputPortRole,
) -> Option<CurrentOutputDependency> {
    inputs
        .iter()
        .find(|input| input.role == role)
        .map(|input| input.current_output_dependency)
}

const NO_INPUTS: &[InputPortSchema] = &[];
const LEVEL_INPUT: &[InputPortSchema] = &[InputPortSchema::current(
    InputPortRole::Input,
    SignalKind::Level,
)];
const LEVEL_SELECT_INPUTS: &[InputPortSchema] = &[
    InputPortSchema::current(InputPortRole::Selector, SignalKind::Level),
    InputPortSchema::current(InputPortRole::WhenLow, SignalKind::Level),
    InputPortSchema::current(InputPortRole::WhenHigh, SignalKind::Level),
];
const PULSE_INPUT: InputPortSchema =
    InputPortSchema::current(InputPortRole::Input, SignalKind::Pulse);
const PULSE_GATE_INPUTS: &[InputPortSchema] = &[
    InputPortSchema::current(InputPortRole::Pulses, SignalKind::Pulse),
    InputPortSchema::current(InputPortRole::Enable, SignalKind::Level),
];
const PULSE_SELECT_INPUTS: &[InputPortSchema] = &[
    InputPortSchema::current(InputPortRole::Selector, SignalKind::Level),
    InputPortSchema::current(InputPortRole::WhenLow, SignalKind::Pulse),
    InputPortSchema::current(InputPortRole::WhenHigh, SignalKind::Pulse),
];
const PULSE_ROUTE_INPUTS: &[InputPortSchema] = &[
    InputPortSchema::current(InputPortRole::Selector, SignalKind::Level),
    InputPortSchema::current(InputPortRole::Pulses, SignalKind::Pulse),
];
const TOGGLE_INPUT: &[InputPortSchema] = &[InputPortSchema::current(
    InputPortRole::Toggle,
    SignalKind::Pulse,
)];
const PULSE_SET_RESET_INPUTS: &[InputPortSchema] = &[
    InputPortSchema::current(InputPortRole::Set, SignalKind::Pulse),
    InputPortSchema::current(InputPortRole::Reset, SignalKind::Pulse),
];
const LEVEL_SET_RESET_INPUTS: &[InputPortSchema] = &[
    InputPortSchema::current(InputPortRole::Set, SignalKind::Level),
    InputPortSchema::current(InputPortRole::Reset, SignalKind::Level),
];
const SAMPLE_HOLD_INPUTS: &[InputPortSchema] = &[
    InputPortSchema::current(InputPortRole::Value, SignalKind::Level),
    InputPortSchema::current(InputPortRole::Sample, SignalKind::Pulse),
];
const PULSE_DELAY_INPUT: &[InputPortSchema] = &[InputPortSchema::future_only(
    InputPortRole::PulseDelay,
    SignalKind::Pulse,
)];
const TRANSPORT_DELAY_INPUT: &[InputPortSchema] = &[InputPortSchema::future_only(
    InputPortRole::TransportDelay,
    SignalKind::Level,
)];

const INERTIAL_DELAY_INPUT: &[InputPortSchema] = &[InputPortSchema::future_only(
    InputPortRole::InertialDelay,
    SignalKind::Level,
)];
const PERIODIC_ENABLE_INPUT: &[InputPortSchema] = &[InputPortSchema::current(
    InputPortRole::Enable,
    SignalKind::Level,
)];

const LEVEL_OUTPUT: &[OutputPortSchema] = &[OutputPortSchema::new(
    OutputPortRole::Output,
    SignalKind::Level,
)];
const PULSE_OUTPUT: &[OutputPortSchema] = &[OutputPortSchema::new(
    OutputPortRole::Output,
    SignalKind::Pulse,
)];
const PULSE_ROUTE_OUTPUTS: &[OutputPortSchema] = &[
    OutputPortSchema::new(OutputPortRole::WhenLow, SignalKind::Pulse),
    OutputPortSchema::new(OutputPortRole::WhenHigh, SignalKind::Pulse),
];

const fn fixed(
    semantic_kind: SemanticNodeKind,
    inputs: &'static [InputPortSchema],
    outputs: &'static [OutputPortSchema],
) -> NodeSchema {
    NodeSchema {
        semantic_kind,
        inputs: InputShape::Fixed(inputs),
        outputs,
        state_family: None,
        temporal_family: None,
    }
}

const fn variadic(
    semantic_kind: SemanticNodeKind,
    input: InputPortSchema,
    minimum: usize,
    outputs: &'static [OutputPortSchema],
) -> NodeSchema {
    NodeSchema {
        semantic_kind,
        inputs: InputShape::Variadic {
            port: input,
            minimum,
        },
        outputs,
        state_family: None,
        temporal_family: None,
    }
}

pub(crate) const fn schema_for_kind(kind: SemanticNodeKind) -> NodeSchema {
    match kind {
        SemanticNodeKind::Constant => fixed(kind, NO_INPUTS, LEVEL_OUTPUT),
        SemanticNodeKind::Not => fixed(kind, LEVEL_INPUT, LEVEL_OUTPUT),
        SemanticNodeKind::All
        | SemanticNodeKind::Any
        | SemanticNodeKind::Parity
        | SemanticNodeKind::AtLeast => variadic(
            kind,
            InputPortSchema::current(InputPortRole::Input, SignalKind::Level),
            0,
            LEVEL_OUTPUT,
        ),
        SemanticNodeKind::Select => fixed(kind, LEVEL_SELECT_INPUTS, LEVEL_OUTPUT),
        SemanticNodeKind::Merge => variadic(kind, PULSE_INPUT, 0, PULSE_OUTPUT),
        SemanticNodeKind::Coalesce => fixed(kind, &[PULSE_INPUT], PULSE_OUTPUT),
        SemanticNodeKind::Zip => variadic(kind, PULSE_INPUT, 1, PULSE_OUTPUT),
        SemanticNodeKind::PulseGate => fixed(kind, PULSE_GATE_INPUTS, PULSE_OUTPUT),
        SemanticNodeKind::PulseSelect => fixed(kind, PULSE_SELECT_INPUTS, PULSE_OUTPUT),
        SemanticNodeKind::PulseRoute => fixed(kind, PULSE_ROUTE_INPUTS, PULSE_ROUTE_OUTPUTS),
        SemanticNodeKind::RisingEdge
        | SemanticNodeKind::FallingEdge
        | SemanticNodeKind::AnyEdge => NodeSchema {
            state_family: Some(StateFamily::EdgeObservation),
            ..fixed(kind, LEVEL_INPUT, PULSE_OUTPUT)
        },
        SemanticNodeKind::Toggle => NodeSchema {
            state_family: Some(StateFamily::StoredLevel),
            ..fixed(kind, TOGGLE_INPUT, LEVEL_OUTPUT)
        },
        SemanticNodeKind::PulseSetResetLatch => NodeSchema {
            state_family: Some(StateFamily::StoredLevel),
            ..fixed(kind, PULSE_SET_RESET_INPUTS, LEVEL_OUTPUT)
        },
        SemanticNodeKind::LevelSetResetLatch => NodeSchema {
            state_family: Some(StateFamily::StoredLevel),
            ..fixed(kind, LEVEL_SET_RESET_INPUTS, LEVEL_OUTPUT)
        },
        SemanticNodeKind::SampleHold => NodeSchema {
            state_family: Some(StateFamily::StoredLevel),
            ..fixed(kind, SAMPLE_HOLD_INPUTS, LEVEL_OUTPUT)
        },
        SemanticNodeKind::PulseDelay => NodeSchema {
            temporal_family: Some(TemporalFamily::PendingPulseGroup),
            ..fixed(kind, PULSE_DELAY_INPUT, PULSE_OUTPUT)
        },
        SemanticNodeKind::TransportDelay => NodeSchema {
            state_family: Some(StateFamily::TransportLevel),
            temporal_family: Some(TemporalFamily::PendingTransportTransition),
            ..fixed(kind, TRANSPORT_DELAY_INPUT, LEVEL_OUTPUT)
        },
        SemanticNodeKind::InertialDelay => NodeSchema {
            state_family: Some(StateFamily::TransportLevel),
            temporal_family: Some(TemporalFamily::InertialCandidate),
            ..fixed(kind, INERTIAL_DELAY_INPUT, LEVEL_OUTPUT)
        },
        SemanticNodeKind::Periodic => NodeSchema {
            state_family: Some(StateFamily::PeriodicEnable),
            temporal_family: Some(TemporalFamily::PeriodicBoundary),
            ..fixed(kind, PERIODIC_ENABLE_INPUT, PULSE_OUTPUT)
        },
    }
}

// SPEC: docs/specs/contracts/current-reaction-dependency-graph.yaml
// "conservative-static-signatures" — every closed kind selects one schema,
// and dependency construction consumes the schema rather than the node name.
pub(crate) const fn node_schema<D>(kind: &NodeKind<D>) -> NodeSchema {
    let semantic_kind = match kind {
        NodeKind::Constant(_) => SemanticNodeKind::Constant,
        NodeKind::Not => SemanticNodeKind::Not,
        NodeKind::All => SemanticNodeKind::All,
        NodeKind::Any => SemanticNodeKind::Any,
        NodeKind::Parity => SemanticNodeKind::Parity,
        NodeKind::AtLeast(_) => SemanticNodeKind::AtLeast,
        NodeKind::Select => SemanticNodeKind::Select,
        NodeKind::Merge => SemanticNodeKind::Merge,
        NodeKind::Coalesce => SemanticNodeKind::Coalesce,
        NodeKind::Zip => SemanticNodeKind::Zip,
        NodeKind::PulseGate => SemanticNodeKind::PulseGate,
        NodeKind::PulseSelect => SemanticNodeKind::PulseSelect,
        NodeKind::PulseRoute => SemanticNodeKind::PulseRoute,
        NodeKind::RisingEdge(_) => SemanticNodeKind::RisingEdge,
        NodeKind::FallingEdge(_) => SemanticNodeKind::FallingEdge,
        NodeKind::AnyEdge(_) => SemanticNodeKind::AnyEdge,
        NodeKind::Toggle(_) => SemanticNodeKind::Toggle,
        NodeKind::PulseSetResetLatch(_) => SemanticNodeKind::PulseSetResetLatch,
        NodeKind::LevelSetResetLatch(_) => SemanticNodeKind::LevelSetResetLatch,
        NodeKind::SampleHold(_) => SemanticNodeKind::SampleHold,
        NodeKind::PulseDelay(_) => SemanticNodeKind::PulseDelay,
        NodeKind::TransportDelay(_) => SemanticNodeKind::TransportDelay,
        NodeKind::InertialDelay(_) => SemanticNodeKind::InertialDelay,
        NodeKind::Periodic(_) => SemanticNodeKind::Periodic,
    };
    schema_for_kind(semantic_kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authored::{EdgeConfig, EdgeInitialization, NodeKind};
    use crate::signal::LogicLevel;
    use crate::time::NonZeroSpan;
    use std::collections::BTreeSet;

    fn every_kind() -> Vec<NodeKind<()>> {
        let delay = NonZeroSpan::from_ticks(1)
            .unwrap_or_else(|_| panic!("positive fixture delay must be valid"));
        vec![
            NodeKind::constant(LogicLevel::Low),
            NodeKind::not(),
            NodeKind::all(),
            NodeKind::any(),
            NodeKind::parity(),
            NodeKind::at_least(1),
            NodeKind::select(),
            NodeKind::merge(),
            NodeKind::coalesce(),
            NodeKind::zip(),
            NodeKind::pulse_gate(),
            NodeKind::pulse_select(),
            NodeKind::pulse_route(),
            NodeKind::rising_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            NodeKind::falling_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            NodeKind::any_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            NodeKind::toggle(LogicLevel::Low),
            NodeKind::pulse_set_reset_latch(crate::authored::PulseSetResetConfig::new(
                LogicLevel::Low,
                crate::authored::ConflictPolicy::SetDominant,
            )),
            NodeKind::level_set_reset_latch(crate::authored::LevelSetResetConfig::new(
                LogicLevel::Low,
                crate::authored::ConflictPolicy::SetDominant,
            )),
            NodeKind::sample_hold(crate::SampleHoldConfig::new(LogicLevel::Low)),
            NodeKind::pulse_delay(delay),
            NodeKind::transport_delay(delay, LogicLevel::Low),
            NodeKind::inertial_delay(delay, LogicLevel::Low),
            NodeKind::periodic(crate::PeriodicConfig::new(
                delay,
                crate::FirstEmissionPolicy::Immediate,
                crate::ReenablePhasePolicy::RestartPhase,
            )),
        ]
    }

    struct SchemaCase {
        kind: NodeKind<()>,
        expected_input_count: Option<usize>,
        minimum_input_count: usize,
        inputs: Vec<(InputPortRole, SignalKind, bool)>,
        outputs: Vec<(OutputPortRole, SignalKind)>,
        state: Option<StateFamily>,
        temporal: Option<TemporalFamily>,
    }

    fn schema_cases() -> Vec<SchemaCase> {
        let fixed = |kind: NodeKind<()>,
                     inputs: Vec<(InputPortRole, SignalKind, bool)>,
                     outputs: Vec<(OutputPortRole, SignalKind)>| SchemaCase {
            kind,
            expected_input_count: Some(inputs.len()),
            minimum_input_count: inputs.len(),
            inputs,
            outputs,
            state: None,
            temporal: None,
        };
        let variadic = |kind: NodeKind<()>,
                        input: (InputPortRole, SignalKind, bool),
                        minimum: usize,
                        output: (OutputPortRole, SignalKind)| SchemaCase {
            kind,
            expected_input_count: None,
            minimum_input_count: minimum,
            inputs: vec![input],
            outputs: vec![output],
            state: None,
            temporal: None,
        };
        let level_input = || vec![(InputPortRole::Input, SignalKind::Level, true)];
        let level_output = || vec![(OutputPortRole::Output, SignalKind::Level)];
        let pulse_output = || vec![(OutputPortRole::Output, SignalKind::Pulse)];
        let delay = NonZeroSpan::from_ticks(1)
            .unwrap_or_else(|_| panic!("positive fixture delay must be valid"));

        let mut rising = fixed(
            NodeKind::rising_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            level_input(),
            pulse_output(),
        );
        rising.state = Some(StateFamily::EdgeObservation);
        let mut falling = fixed(
            NodeKind::falling_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            level_input(),
            pulse_output(),
        );
        falling.state = Some(StateFamily::EdgeObservation);
        let mut any_edge = fixed(
            NodeKind::any_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            level_input(),
            pulse_output(),
        );
        any_edge.state = Some(StateFamily::EdgeObservation);
        let mut toggle = fixed(
            NodeKind::toggle(LogicLevel::Low),
            vec![(InputPortRole::Toggle, SignalKind::Pulse, true)],
            level_output(),
        );
        toggle.state = Some(StateFamily::StoredLevel);
        let mut pulse_latch = fixed(
            NodeKind::pulse_set_reset_latch(crate::authored::PulseSetResetConfig::new(
                LogicLevel::Low,
                crate::authored::ConflictPolicy::SetDominant,
            )),
            vec![
                (InputPortRole::Set, SignalKind::Pulse, true),
                (InputPortRole::Reset, SignalKind::Pulse, true),
            ],
            level_output(),
        );
        pulse_latch.state = Some(StateFamily::StoredLevel);
        let mut level_latch = fixed(
            NodeKind::level_set_reset_latch(crate::authored::LevelSetResetConfig::new(
                LogicLevel::Low,
                crate::authored::ConflictPolicy::SetDominant,
            )),
            vec![
                (InputPortRole::Set, SignalKind::Level, true),
                (InputPortRole::Reset, SignalKind::Level, true),
            ],
            level_output(),
        );
        level_latch.state = Some(StateFamily::StoredLevel);
        let mut sample_hold = fixed(
            NodeKind::sample_hold(crate::SampleHoldConfig::new(LogicLevel::Low)),
            vec![
                (InputPortRole::Value, SignalKind::Level, true),
                (InputPortRole::Sample, SignalKind::Pulse, true),
            ],
            level_output(),
        );
        sample_hold.state = Some(StateFamily::StoredLevel);
        let mut pulse_delay = fixed(
            NodeKind::pulse_delay(delay),
            vec![(InputPortRole::PulseDelay, SignalKind::Pulse, false)],
            pulse_output(),
        );
        pulse_delay.temporal = Some(TemporalFamily::PendingPulseGroup);
        let mut transport_delay = fixed(
            NodeKind::transport_delay(delay, LogicLevel::Low),
            vec![(InputPortRole::TransportDelay, SignalKind::Level, false)],
            level_output(),
        );
        transport_delay.state = Some(StateFamily::TransportLevel);
        transport_delay.temporal = Some(TemporalFamily::PendingTransportTransition);
        let mut inertial_delay = fixed(
            NodeKind::inertial_delay(delay, LogicLevel::Low),
            vec![(InputPortRole::InertialDelay, SignalKind::Level, false)],
            level_output(),
        );
        inertial_delay.state = Some(StateFamily::TransportLevel);
        inertial_delay.temporal = Some(TemporalFamily::InertialCandidate);
        let mut periodic = fixed(
            NodeKind::periodic(crate::PeriodicConfig::new(
                delay,
                crate::FirstEmissionPolicy::Immediate,
                crate::ReenablePhasePolicy::RestartPhase,
            )),
            vec![(InputPortRole::Enable, SignalKind::Level, true)],
            pulse_output(),
        );
        periodic.state = Some(StateFamily::PeriodicEnable);
        periodic.temporal = Some(TemporalFamily::PeriodicBoundary);

        vec![
            fixed(
                NodeKind::constant(LogicLevel::Low),
                Vec::new(),
                level_output(),
            ),
            fixed(NodeKind::not(), level_input(), level_output()),
            variadic(
                NodeKind::all(),
                (InputPortRole::Input, SignalKind::Level, true),
                0,
                (OutputPortRole::Output, SignalKind::Level),
            ),
            variadic(
                NodeKind::any(),
                (InputPortRole::Input, SignalKind::Level, true),
                0,
                (OutputPortRole::Output, SignalKind::Level),
            ),
            variadic(
                NodeKind::parity(),
                (InputPortRole::Input, SignalKind::Level, true),
                0,
                (OutputPortRole::Output, SignalKind::Level),
            ),
            variadic(
                NodeKind::at_least(1),
                (InputPortRole::Input, SignalKind::Level, true),
                0,
                (OutputPortRole::Output, SignalKind::Level),
            ),
            fixed(
                NodeKind::select(),
                vec![
                    (InputPortRole::Selector, SignalKind::Level, true),
                    (InputPortRole::WhenLow, SignalKind::Level, true),
                    (InputPortRole::WhenHigh, SignalKind::Level, true),
                ],
                level_output(),
            ),
            variadic(
                NodeKind::merge(),
                (InputPortRole::Input, SignalKind::Pulse, true),
                0,
                (OutputPortRole::Output, SignalKind::Pulse),
            ),
            fixed(
                NodeKind::coalesce(),
                vec![(InputPortRole::Input, SignalKind::Pulse, true)],
                pulse_output(),
            ),
            variadic(
                NodeKind::zip(),
                (InputPortRole::Input, SignalKind::Pulse, true),
                1,
                (OutputPortRole::Output, SignalKind::Pulse),
            ),
            fixed(
                NodeKind::pulse_gate(),
                vec![
                    (InputPortRole::Pulses, SignalKind::Pulse, true),
                    (InputPortRole::Enable, SignalKind::Level, true),
                ],
                pulse_output(),
            ),
            fixed(
                NodeKind::pulse_select(),
                vec![
                    (InputPortRole::Selector, SignalKind::Level, true),
                    (InputPortRole::WhenLow, SignalKind::Pulse, true),
                    (InputPortRole::WhenHigh, SignalKind::Pulse, true),
                ],
                pulse_output(),
            ),
            fixed(
                NodeKind::pulse_route(),
                vec![
                    (InputPortRole::Selector, SignalKind::Level, true),
                    (InputPortRole::Pulses, SignalKind::Pulse, true),
                ],
                vec![
                    (OutputPortRole::WhenLow, SignalKind::Pulse),
                    (OutputPortRole::WhenHigh, SignalKind::Pulse),
                ],
            ),
            rising,
            falling,
            any_edge,
            toggle,
            pulse_latch,
            level_latch,
            sample_hold,
            pulse_delay,
            transport_delay,
            inertial_delay,
            periodic,
        ]
    }

    #[test]
    fn every_closed_node_kind_has_one_distinct_schema_identity() {
        let schemas: Vec<_> = every_kind().iter().map(node_schema).collect();
        assert_eq!(schemas.len(), 24);
        assert_eq!(
            schemas
                .iter()
                .map(|schema| schema.semantic_kind())
                .collect::<BTreeSet<_>>()
                .len(),
            schemas.len()
        );
        assert_eq!(
            schemas
                .iter()
                .map(|schema| schema.semantic_kind().identity_tag())
                .collect::<BTreeSet<_>>()
                .len(),
            schemas.len()
        );
    }

    #[test]
    fn every_closed_node_kind_matches_the_complete_schema_matrix() {
        let cases = schema_cases();
        assert_eq!(cases.len(), 24);
        for case in cases {
            let schema = node_schema(&case.kind);
            assert_eq!(schema.expected_input_count(), case.expected_input_count);
            assert_eq!(schema.minimum_input_count(), case.minimum_input_count);
            assert_eq!(schema.expected_output_count(), case.outputs.len());
            assert_eq!(schema.state_family(), case.state);
            assert_eq!(schema.temporal_family(), case.temporal);

            let claimed_input_count = case.expected_input_count.unwrap_or(2);
            let required_input_roles = schema.required_input_roles(claimed_input_count);
            if case.expected_input_count.is_some() {
                assert_eq!(
                    required_input_roles,
                    case.inputs.iter().map(|input| input.0).collect::<Vec<_>>()
                );
            } else {
                assert_eq!(required_input_roles, vec![case.inputs[0].0; 2]);
            }
            let output_roles: Vec<_> = case.outputs.iter().map(|output| output.0).collect();
            assert_eq!(
                schema.required_output_roles(case.outputs.len()),
                output_roles
            );
            for (role, kind, current) in case.inputs {
                assert_eq!(schema.input_kind(role), Some(kind));
                assert_eq!(
                    schema.input_affects_any_current_output(role, &output_roles),
                    current
                );
            }
            for (role, kind) in case.outputs {
                assert_eq!(schema.output_kind(role), Some(kind));
                assert!(schema.accepts_output_kind(kind));
            }
        }
    }

    #[test]
    fn edge_family_shares_shape_state_and_immediate_dependency() {
        for kind in [
            NodeKind::<()>::rising_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            NodeKind::falling_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
            NodeKind::any_edge(EdgeConfig::new(EdgeInitialization::Baseline)),
        ] {
            let schema = node_schema(&kind);
            assert_eq!(schema.expected_input_count(), Some(1));
            assert_eq!(
                schema.input_kind(InputPortRole::Input),
                Some(SignalKind::Level)
            );
            assert_eq!(schema.expected_output_count(), 1);
            assert_eq!(
                schema.output_kind(OutputPortRole::Output),
                Some(SignalKind::Pulse)
            );
            assert_eq!(schema.state_family(), Some(StateFamily::EdgeObservation));
            assert!(
                schema.input_affects_any_current_output(
                    InputPortRole::Input,
                    &[OutputPortRole::Output]
                )
            );
        }
    }

    #[test]
    fn pulse_delay_is_future_only_while_toggle_is_immediate() {
        let delay = NonZeroSpan::from_ticks(1)
            .unwrap_or_else(|_| panic!("positive fixture delay must be valid"));
        let pulse_delay = node_schema(&NodeKind::<()>::pulse_delay(delay));
        assert_eq!(
            pulse_delay.temporal_family(),
            Some(TemporalFamily::PendingPulseGroup)
        );
        assert!(!pulse_delay.input_affects_any_current_output(
            InputPortRole::PulseDelay,
            &[OutputPortRole::Output]
        ));

        let toggle = node_schema(&NodeKind::<()>::toggle(LogicLevel::Low));
        assert_eq!(toggle.state_family(), Some(StateFamily::StoredLevel));
        assert!(
            toggle
                .input_affects_any_current_output(InputPortRole::Toggle, &[OutputPortRole::Output])
        );
    }
}
