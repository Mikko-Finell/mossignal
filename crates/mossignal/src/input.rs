//! Complete and incremental external Level and Pulse input artifacts.

use crate::identity::{InputSchemaFingerprint, NetworkFingerprint};
use crate::key::{AnyExternalInputKey, ExternalInputKey, NetworkKey};
use crate::signal::{Level, LogicLevel, Pulse, PulseCount, SignalKind};
use core::fmt;
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};

/// An owned complete external-input batch for one compiled network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSnapshot<D> {
    network_key: NetworkKey,
    network_fingerprint: NetworkFingerprint,
    input_schema_fingerprint: InputSchemaFingerprint,
    pub(crate) levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pub(crate) pulses: BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    domain: PhantomData<fn() -> D>,
}

impl<D> InputSnapshot<D> {
    /// Returns the stable network identity for which this snapshot was built.
    #[must_use]
    pub const fn network_key(&self) -> NetworkKey {
        self.network_key
    }

    /// Returns the exact compiled-network identity for which this snapshot was built.
    #[must_use]
    pub const fn network_fingerprint(&self) -> NetworkFingerprint {
        self.network_fingerprint
    }

    /// Returns the exact external-input schema identity for which this snapshot was built.
    #[must_use]
    pub const fn input_schema_fingerprint(&self) -> InputSchemaFingerprint {
        self.input_schema_fingerprint
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    ) {
        (self.levels, self.pulses)
    }
}

/// An owned builder for a complete external-input snapshot.
#[derive(Debug)]
pub struct InputSnapshotBuilder<D> {
    network_key: NetworkKey,
    network_fingerprint: NetworkFingerprint,
    input_schema_fingerprint: InputSchemaFingerprint,
    required_inputs: BTreeSet<ExternalInputKey<Level>>,
    pulse_inputs: BTreeSet<ExternalInputKey<Pulse>>,
    levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    domain: PhantomData<fn() -> D>,
}

impl<D> InputSnapshotBuilder<D> {
    pub(crate) fn new(
        network_key: NetworkKey,
        network_fingerprint: NetworkFingerprint,
        input_schema_fingerprint: InputSchemaFingerprint,
        required_inputs: impl IntoIterator<Item = ExternalInputKey<Level>>,
        pulse_inputs: impl IntoIterator<Item = ExternalInputKey<Pulse>>,
    ) -> Self {
        Self {
            network_key,
            network_fingerprint,
            input_schema_fingerprint,
            required_inputs: required_inputs.into_iter().collect(),
            pulse_inputs: pulse_inputs.into_iter().collect(),
            levels: BTreeMap::new(),
            pulses: BTreeMap::new(),
            domain: PhantomData,
        }
    }

    /// Adds one authoritative external level observation.
    pub fn set(
        mut self,
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    ) -> Result<Self, InputBuildFailure> {
        if !self.required_inputs.contains(&input) {
            if self
                .pulse_inputs
                .iter()
                .any(|pulse| pulse.as_u128() == input.as_u128())
            {
                return Err(InputBuildFailure::WrongSignalKind {
                    input: input.into(),
                    expected: SignalKind::Pulse,
                    actual: SignalKind::Level,
                });
            }
            return Err(InputBuildFailure::UnknownInput { input });
        }

        if let Some(previous) = self.levels.insert(input, value) {
            let failure = if previous == value {
                InputBuildFailure::DuplicateObservation { input, value }
            } else {
                InputBuildFailure::ConflictingObservation {
                    input,
                    first: previous,
                    second: value,
                }
            };
            return Err(failure);
        }

        Ok(self)
    }

    /// Adds one reaction-scoped external pulse observation.
    pub fn pulse(
        mut self,
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    ) -> Result<Self, InputBuildFailure> {
        if !self.pulse_inputs.contains(&input) {
            if self
                .required_inputs
                .iter()
                .any(|level| level.as_u128() == input.as_u128())
            {
                return Err(InputBuildFailure::WrongSignalKind {
                    input: input.into(),
                    expected: SignalKind::Level,
                    actual: SignalKind::Pulse,
                });
            }
            return Err(InputBuildFailure::UnknownPulseInput { input });
        }
        if let Some(previous) = self.pulses.insert(input, count) {
            return Err(if previous == count {
                InputBuildFailure::DuplicatePulseObservation { input, count }
            } else {
                InputBuildFailure::ConflictingPulseObservation {
                    input,
                    first: previous,
                    second: count,
                }
            });
        }
        Ok(self)
    }

    /// Completes a snapshot only when every required level has one value.
    pub fn finish(self) -> Result<InputSnapshot<D>, InputBuildFailure> {
        let missing = self
            .required_inputs
            .iter()
            .filter(|input| !self.levels.contains_key(input))
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(InputBuildFailure::MissingRequiredLevels { missing });
        }

        Ok(InputSnapshot {
            network_key: self.network_key,
            network_fingerprint: self.network_fingerprint,
            input_schema_fingerprint: self.input_schema_fingerprint,
            levels: self.levels,
            pulses: self.pulses,
            domain: PhantomData,
        })
    }
}

/// An owned set of explicit level observations for a ready machine.
///
/// Omitted inputs have no replacement value in this artifact. A later
/// compatible ready-machine transaction retains their prior authoritative
/// values.
pub struct InputDelta<D> {
    network_key: NetworkKey,
    network_fingerprint: NetworkFingerprint,
    input_schema_fingerprint: InputSchemaFingerprint,
    pub(crate) levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pub(crate) pulses: BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    domain: PhantomData<fn() -> D>,
}

impl<D> Clone for InputDelta<D> {
    fn clone(&self) -> Self {
        Self {
            network_key: self.network_key,
            network_fingerprint: self.network_fingerprint,
            input_schema_fingerprint: self.input_schema_fingerprint,
            levels: self.levels.clone(),
            pulses: self.pulses.clone(),
            domain: PhantomData,
        }
    }
}

impl<D> fmt::Debug for InputDelta<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InputDelta")
            .field("network_key", &self.network_key)
            .field("network_fingerprint", &self.network_fingerprint)
            .field("input_schema_fingerprint", &self.input_schema_fingerprint)
            .field("levels", &self.levels)
            .field("pulses", &self.pulses)
            .finish()
    }
}

impl<D> PartialEq for InputDelta<D> {
    fn eq(&self, other: &Self) -> bool {
        self.network_key == other.network_key
            && self.network_fingerprint == other.network_fingerprint
            && self.input_schema_fingerprint == other.input_schema_fingerprint
            && self.levels == other.levels
            && self.pulses == other.pulses
    }
}

impl<D> Eq for InputDelta<D> {}

impl<D> InputDelta<D> {
    /// Returns the stable network identity for which this delta was built.
    #[must_use]
    pub const fn network_key(&self) -> NetworkKey {
        self.network_key
    }

    /// Returns the exact compiled-network identity for which this delta was built.
    #[must_use]
    pub const fn network_fingerprint(&self) -> NetworkFingerprint {
        self.network_fingerprint
    }

    /// Returns the exact external-input schema identity for which this delta was built.
    #[must_use]
    pub const fn input_schema_fingerprint(&self) -> InputSchemaFingerprint {
        self.input_schema_fingerprint
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        BTreeMap<ExternalInputKey<Level>, LogicLevel>,
        BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    ) {
        (self.levels, self.pulses)
    }

    #[cfg(test)]
    pub(crate) fn with_test_bindings(
        mut self,
        network_fingerprint: NetworkFingerprint,
        input_schema_fingerprint: InputSchemaFingerprint,
    ) -> Self {
        self.network_fingerprint = network_fingerprint;
        self.input_schema_fingerprint = input_schema_fingerprint;
        self
    }
}

/// An owned builder for explicit observations against one current topology.
pub struct InputDeltaBuilder<D> {
    network_key: NetworkKey,
    network_fingerprint: NetworkFingerprint,
    input_schema_fingerprint: InputSchemaFingerprint,
    existing_inputs: BTreeSet<ExternalInputKey<Level>>,
    pulse_inputs: BTreeSet<ExternalInputKey<Pulse>>,
    levels: BTreeMap<ExternalInputKey<Level>, LogicLevel>,
    pulses: BTreeMap<ExternalInputKey<Pulse>, PulseCount>,
    domain: PhantomData<fn() -> D>,
}

impl<D> fmt::Debug for InputDeltaBuilder<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InputDeltaBuilder")
            .field("network_key", &self.network_key)
            .field("network_fingerprint", &self.network_fingerprint)
            .field("input_schema_fingerprint", &self.input_schema_fingerprint)
            .field("existing_inputs", &self.existing_inputs)
            .field("pulse_inputs", &self.pulse_inputs)
            .field("levels", &self.levels)
            .field("pulses", &self.pulses)
            .finish()
    }
}

impl<D> InputDeltaBuilder<D> {
    pub(crate) fn new(
        network_key: NetworkKey,
        network_fingerprint: NetworkFingerprint,
        input_schema_fingerprint: InputSchemaFingerprint,
        existing_inputs: impl IntoIterator<Item = ExternalInputKey<Level>>,
        pulse_inputs: impl IntoIterator<Item = ExternalInputKey<Pulse>>,
    ) -> Self {
        Self {
            network_key,
            network_fingerprint,
            input_schema_fingerprint,
            existing_inputs: existing_inputs.into_iter().collect(),
            pulse_inputs: pulse_inputs.into_iter().collect(),
            levels: BTreeMap::new(),
            pulses: BTreeMap::new(),
            domain: PhantomData,
        }
    }

    /// Explicitly changes or reasserts one existing external level input.
    pub fn set(
        mut self,
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    ) -> Result<Self, InputBuildFailure> {
        if !self.existing_inputs.contains(&input) {
            if self
                .pulse_inputs
                .iter()
                .any(|pulse| pulse.as_u128() == input.as_u128())
            {
                return Err(InputBuildFailure::WrongSignalKind {
                    input: input.into(),
                    expected: SignalKind::Pulse,
                    actual: SignalKind::Level,
                });
            }
            return Err(InputBuildFailure::UnknownInput { input });
        }

        if let Some(previous) = self.levels.insert(input, value) {
            let failure = if previous == value {
                InputBuildFailure::DuplicateObservation { input, value }
            } else {
                InputBuildFailure::ConflictingObservation {
                    input,
                    first: previous,
                    second: value,
                }
            };
            return Err(failure);
        }

        Ok(self)
    }

    /// Adds one reaction-scoped external pulse observation.
    pub fn pulse(
        mut self,
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    ) -> Result<Self, InputBuildFailure> {
        if !self.pulse_inputs.contains(&input) {
            if self
                .existing_inputs
                .iter()
                .any(|level| level.as_u128() == input.as_u128())
            {
                return Err(InputBuildFailure::WrongSignalKind {
                    input: input.into(),
                    expected: SignalKind::Level,
                    actual: SignalKind::Pulse,
                });
            }
            return Err(InputBuildFailure::UnknownPulseInput { input });
        }
        if let Some(previous) = self.pulses.insert(input, count) {
            return Err(if previous == count {
                InputBuildFailure::DuplicatePulseObservation { input, count }
            } else {
                InputBuildFailure::ConflictingPulseObservation {
                    input,
                    first: previous,
                    second: count,
                }
            });
        }
        Ok(self)
    }

    /// Completes this delta without requiring omitted existing level inputs.
    pub fn finish(self) -> Result<InputDelta<D>, InputBuildFailure> {
        Ok(InputDelta {
            network_key: self.network_key,
            network_fingerprint: self.network_fingerprint,
            input_schema_fingerprint: self.input_schema_fingerprint,
            levels: self.levels,
            pulses: self.pulses,
            domain: PhantomData,
        })
    }
}

/// A failure while constructing a level input artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputBuildFailure {
    /// An observation named an external level input outside the bound schema.
    UnknownInput { input: ExternalInputKey<Level> },
    /// An observation named an external pulse input outside the bound schema.
    UnknownPulseInput { input: ExternalInputKey<Pulse> },
    /// An observation used a typed key whose payload belongs to the other signal kind.
    WrongSignalKind {
        input: AnyExternalInputKey,
        expected: SignalKind,
        actual: SignalKind,
    },
    /// An equivalent observation was supplied more than once.
    DuplicateObservation {
        input: ExternalInputKey<Level>,
        value: LogicLevel,
    },
    /// Two observations for one input supplied different values.
    ConflictingObservation {
        input: ExternalInputKey<Level>,
        first: LogicLevel,
        second: LogicLevel,
    },
    /// An equivalent pulse observation was supplied more than once.
    DuplicatePulseObservation {
        input: ExternalInputKey<Pulse>,
        count: PulseCount,
    },
    /// Two pulse observations for one input supplied different counts.
    ConflictingPulseObservation {
        input: ExternalInputKey<Pulse>,
        first: PulseCount,
        second: PulseCount,
    },
    /// Completion omitted one or more required external level inputs.
    MissingRequiredLevels {
        missing: Vec<ExternalInputKey<Level>>,
    },
}

impl fmt::Display for InputBuildFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownInput { .. } | Self::UnknownPulseInput { .. } => {
                formatter.write_str("input is outside the bound schema")
            }
            Self::WrongSignalKind { .. } => {
                formatter.write_str("input observation uses the wrong signal kind")
            }
            Self::DuplicateObservation { .. } => {
                formatter.write_str("input was observed more than once with the same value")
            }
            Self::ConflictingObservation { .. } => {
                formatter.write_str("input was observed more than once with conflicting values")
            }
            Self::DuplicatePulseObservation { .. } => {
                formatter.write_str("pulse input was observed more than once with the same count")
            }
            Self::ConflictingPulseObservation { .. } => formatter
                .write_str("pulse input was observed more than once with conflicting counts"),
            Self::MissingRequiredLevels { .. } => {
                formatter.write_str("snapshot omits one or more required level inputs")
            }
        }
    }
}

impl std::error::Error for InputBuildFailure {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeDomainId;
    use crate::authored::{ExternalInputDef, UncheckedNetwork};
    use crate::key::{ExternalInputKey, NetworkKey};
    use crate::metadata::DiagnosticMeta;

    struct UncooperativeDomain;

    fn assert_debug<T: fmt::Debug>() {}

    fn assert_delta_value_traits<T: Clone + fmt::Debug + Eq>() {}

    fn compiled_with_inputs(keys: &[u128]) -> crate::CompiledNetwork<()> {
        compiled_with_identity_and_inputs(1, keys)
    }

    fn compiled_with_identity_and_inputs(
        network_key: u128,
        keys: &[u128],
    ) -> crate::CompiledNetwork<()> {
        compiled_with_meta_and_inputs(network_key, DiagnosticMeta::default(), keys)
    }

    fn compiled_with_meta_and_inputs(
        network_key: u128,
        meta: DiagnosticMeta,
        keys: &[u128],
    ) -> crate::CompiledNetwork<()> {
        let external_inputs = keys
            .iter()
            .copied()
            .map(|key| {
                ExternalInputDef::new(
                    ExternalInputKey::<Level>::from_u128(key).into(),
                    DiagnosticMeta::default(),
                )
            })
            .collect();
        UncheckedNetwork::new(
            NetworkKey::from_u128(network_key),
            TimeDomainId::from_u128(2),
            meta,
            Vec::new(),
            external_inputs,
            Vec::new(),
            Vec::new(),
        )
        .validate()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must validate"))
        .compile()
        .require_artifact()
        .unwrap_or_else(|_| panic!("fixture must compile"))
    }

    #[test]
    fn complete_snapshot_retains_compiled_identities_and_key_order() {
        let compiled = compiled_with_inputs(&[9, 3]);
        let snapshot = compiled
            .input_snapshot()
            .set(ExternalInputKey::from_u128(9), LogicLevel::High)
            .and_then(|builder| builder.set(ExternalInputKey::from_u128(3), LogicLevel::Low))
            .and_then(InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("complete snapshot must build"));
        let reordered = compiled
            .input_snapshot()
            .set(ExternalInputKey::from_u128(3), LogicLevel::Low)
            .and_then(|builder| builder.set(ExternalInputKey::from_u128(9), LogicLevel::High))
            .and_then(InputSnapshotBuilder::finish)
            .unwrap_or_else(|_| panic!("complete snapshot must build"));

        assert_eq!(snapshot.network_fingerprint(), compiled.fingerprint());
        assert_eq!(snapshot.network_key(), compiled.network_key());
        assert_eq!(
            snapshot.input_schema_fingerprint(),
            compiled.input_schema_fingerprint()
        );
        assert_eq!(snapshot, reordered);
        assert_eq!(
            snapshot
                .levels
                .iter()
                .map(|(&key, &value)| (key, value))
                .collect::<Vec<_>>(),
            vec![
                (ExternalInputKey::from_u128(3), LogicLevel::Low),
                (ExternalInputKey::from_u128(9), LogicLevel::High),
            ]
        );
    }

    #[test]
    fn builder_rejects_unknown_duplicate_and_conflicting_observations() {
        let compiled = compiled_with_inputs(&[3]);
        let input = ExternalInputKey::from_u128(3);

        assert_eq!(
            compiled
                .input_snapshot()
                .set(ExternalInputKey::from_u128(4), LogicLevel::High)
                .unwrap_err(),
            InputBuildFailure::UnknownInput {
                input: ExternalInputKey::from_u128(4),
            }
        );
        assert_eq!(
            compiled
                .input_snapshot()
                .set(input, LogicLevel::Low)
                .and_then(|builder| builder.set(input, LogicLevel::Low))
                .unwrap_err(),
            InputBuildFailure::DuplicateObservation {
                input,
                value: LogicLevel::Low,
            }
        );
        assert_eq!(
            compiled
                .input_snapshot()
                .set(input, LogicLevel::Low)
                .and_then(|builder| builder.set(input, LogicLevel::High))
                .unwrap_err(),
            InputBuildFailure::ConflictingObservation {
                input,
                first: LogicLevel::Low,
                second: LogicLevel::High,
            }
        );
    }

    #[test]
    fn finish_rejects_missing_levels_and_accepts_an_empty_schema() {
        let compiled = compiled_with_inputs(&[9, 3]);
        assert_eq!(
            compiled.input_snapshot().finish(),
            Err(InputBuildFailure::MissingRequiredLevels {
                missing: vec![
                    ExternalInputKey::from_u128(3),
                    ExternalInputKey::from_u128(9),
                ],
            })
        );
        assert_eq!(
            compiled
                .input_snapshot()
                .set(ExternalInputKey::from_u128(3), LogicLevel::High)
                .and_then(InputSnapshotBuilder::finish),
            Err(InputBuildFailure::MissingRequiredLevels {
                missing: vec![ExternalInputKey::from_u128(9)],
            })
        );
        assert_eq!(
            compiled
                .input_snapshot()
                .set(ExternalInputKey::from_u128(9), LogicLevel::Low)
                .and_then(InputSnapshotBuilder::finish),
            Err(InputBuildFailure::MissingRequiredLevels {
                missing: vec![ExternalInputKey::from_u128(3)],
            })
        );

        let empty = compiled_with_inputs(&[])
            .input_snapshot()
            .finish()
            .unwrap_or_else(|_| panic!("an empty schema has a complete empty snapshot"));
        assert!(empty.levels.is_empty());
    }

    #[test]
    fn delta_accepts_empty_partial_complete_and_reasserted_levels() {
        let compiled = compiled_with_inputs(&[9, 3]);
        let empty = compiled
            .input_delta()
            .finish()
            .unwrap_or_else(|_| panic!("empty delta must build"));
        let partial = compiled
            .input_delta()
            .set(ExternalInputKey::from_u128(3), LogicLevel::High)
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("partial delta must build"));
        let complete = compiled
            .input_delta()
            .set(ExternalInputKey::from_u128(3), LogicLevel::Low)
            .and_then(|builder| builder.set(ExternalInputKey::from_u128(9), LogicLevel::High))
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("complete delta must build"));

        for delta in [&empty, &partial, &complete] {
            assert_eq!(delta.network_key(), compiled.network_key());
            assert_eq!(delta.network_fingerprint(), compiled.fingerprint());
            assert_eq!(
                delta.input_schema_fingerprint(),
                compiled.input_schema_fingerprint()
            );
        }
        assert!(empty.levels.is_empty());
        assert_eq!(
            partial.levels.get(&ExternalInputKey::from_u128(3)),
            Some(&LogicLevel::High)
        );
        assert!(!partial.levels.contains_key(&ExternalInputKey::from_u128(9)));
        assert_eq!(complete.levels.len(), 2);
    }

    #[test]
    fn delta_traits_do_not_require_domain_traits() {
        assert_delta_value_traits::<InputDelta<UncooperativeDomain>>();
        assert_debug::<InputDeltaBuilder<UncooperativeDomain>>();
    }

    #[test]
    fn delta_rejects_unknown_duplicate_and_conflicting_observations() {
        let compiled = compiled_with_inputs(&[3]);
        let input = ExternalInputKey::from_u128(3);

        assert_eq!(
            compiled
                .input_delta()
                .set(ExternalInputKey::from_u128(4), LogicLevel::High)
                .unwrap_err(),
            InputBuildFailure::UnknownInput {
                input: ExternalInputKey::from_u128(4),
            }
        );
        assert_eq!(
            compiled
                .input_delta()
                .set(input, LogicLevel::Low)
                .and_then(|builder| builder.set(input, LogicLevel::Low))
                .unwrap_err(),
            InputBuildFailure::DuplicateObservation {
                input,
                value: LogicLevel::Low,
            }
        );
        assert_eq!(
            compiled
                .input_delta()
                .set(input, LogicLevel::Low)
                .and_then(|builder| builder.set(input, LogicLevel::High))
                .unwrap_err(),
            InputBuildFailure::ConflictingObservation {
                input,
                first: LogicLevel::Low,
                second: LogicLevel::High,
            }
        );
    }

    #[test]
    fn delta_semantics_are_order_independent_and_exactly_bound() {
        let compiled = compiled_with_inputs(&[9, 3]);
        let first = compiled
            .input_delta()
            .set(ExternalInputKey::from_u128(9), LogicLevel::High)
            .and_then(|builder| builder.set(ExternalInputKey::from_u128(3), LogicLevel::Low))
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("delta must build"));
        let reordered = compiled
            .input_delta()
            .set(ExternalInputKey::from_u128(3), LogicLevel::Low)
            .and_then(|builder| builder.set(ExternalInputKey::from_u128(9), LogicLevel::High))
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("reordered delta must build"));
        assert_eq!(first, reordered);

        let foreign_network = compiled_with_identity_and_inputs(2, &[9, 3])
            .input_delta()
            .set(ExternalInputKey::from_u128(3), LogicLevel::Low)
            .and_then(|builder| builder.set(ExternalInputKey::from_u128(9), LogicLevel::High))
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("foreign-network delta must build"));
        assert_eq!(
            first.input_schema_fingerprint(),
            foreign_network.input_schema_fingerprint()
        );
        assert_ne!(first.network_key(), foreign_network.network_key());
        assert_ne!(
            first.network_fingerprint(),
            foreign_network.network_fingerprint()
        );
        assert_ne!(first, foreign_network);

        let foreign_schema = compiled_with_inputs(&[9, 3, 4])
            .input_delta()
            .set(ExternalInputKey::from_u128(3), LogicLevel::Low)
            .and_then(|builder| builder.set(ExternalInputKey::from_u128(9), LogicLevel::High))
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("foreign-schema delta must build"));
        assert_ne!(
            first.input_schema_fingerprint(),
            foreign_schema.input_schema_fingerprint()
        );
        assert_ne!(first, foreign_schema);
    }

    #[test]
    fn delta_semantics_ignore_diagnostic_metadata() {
        let default = compiled_with_meta_and_inputs(1, DiagnosticMeta::default(), &[3]);
        let annotated = compiled_with_meta_and_inputs(
            1,
            DiagnosticMeta {
                name: Some(String::from("annotated network")),
                tags: vec![String::from("presentation only")],
                ..DiagnosticMeta::default()
            },
            &[3],
        );
        let input = ExternalInputKey::from_u128(3);
        let default_delta = default
            .input_delta()
            .set(input, LogicLevel::High)
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("delta must build"));
        let annotated_delta = annotated
            .input_delta()
            .set(input, LogicLevel::High)
            .and_then(InputDeltaBuilder::finish)
            .unwrap_or_else(|_| panic!("delta must build"));

        assert_eq!(default_delta, annotated_delta);
    }
}
