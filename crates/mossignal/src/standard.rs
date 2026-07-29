//! Immutable standard-module catalogue and canonical `Exactly` construction.

use crate::ModuleDef;
use crate::authored::{
    ConnectionDef, ConnectionEndpoint, ModuleInputDef, ModuleInterfaceMapping, ModuleOutputDef,
    NodeDef, NodeKind, NodePorts, UncheckedModule,
};
use crate::diagnostics::{Diagnostic, DiagnosticSet, Problem, ProblemEvidence, Report, SubjectRef};
use crate::key::{
    AnyModuleInputKey, AnyModuleOutputKey, ConnectionKey, InPortKey, ModuleInputKey,
    ModuleOutputKey, NodeKey, OutPortKey,
};
use crate::metadata::DiagnosticMeta;
use crate::signal::{Level, LogicLevel, SignalKind};
use crate::time::{NonZeroSpan, Span};
use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeSet;

const EXACTLY_ID: &str = "mossignal.standard.exactly";
const PUBLIC_KEY_DOMAIN: &str = "mossignal/standard_module_public_key/v1";
const INTERNAL_KEY_DOMAIN: &str = "mossignal/standard_module_internal_key/v1";
const EXPANSION_FINGERPRINT_DOMAIN: &str = "mossignal/standard_module_expansion_fingerprint/v1";

macro_rules! version_value {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            /// Creates a nonzero version identity.
            #[must_use]
            pub const fn new(value: u32) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }

            /// Returns the exact numeric identity.
            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }

            pub(crate) const fn one() -> Self {
                Self(1)
            }
        }
    };
}

version_value!(
    StandardCatalogueVersion,
    "The exact standard-catalogue version."
);
version_value!(
    StandardModuleSemanticVersion,
    "The public semantic version of one standard descriptor."
);
version_value!(
    StandardModuleExpansionVersion,
    "The canonical primitive-expansion version of one standard descriptor."
);

/// Failure to construct a reserved standard-module identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardModuleIdError;

impl fmt::Display for StandardModuleIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("standard module id must match mossignal.standard.<lowercase_segment>")
    }
}

impl std::error::Error for StandardModuleIdError {}

/// A validated owned identifier in the core standard-module namespace.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardModuleId(String);

impl StandardModuleId {
    /// Validates and owns one reserved standard-module identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, StandardModuleIdError> {
        let value = value.into();
        let Some(suffix) = value.strip_prefix("mossignal.standard.") else {
            return Err(StandardModuleIdError);
        };
        if suffix.is_empty()
            || suffix.split('.').any(|segment| {
                let mut chars = segment.chars();
                !matches!(chars.next(), Some('a'..='z'))
                    || chars.any(|character| !matches!(character, 'a'..='z' | '0'..='9' | '_'))
            })
        {
            return Err(StandardModuleIdError);
        }
        Ok(Self(value))
    }

    /// Returns the stable dotted identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn exactly() -> Self {
        Self(EXACTLY_ID.to_owned())
    }
}

impl fmt::Debug for StandardModuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("StandardModuleId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for StandardModuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Exact identity of one standard-module descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardModuleRef {
    id: StandardModuleId,
    semantic_version: StandardModuleSemanticVersion,
    expansion_version: StandardModuleExpansionVersion,
}

impl StandardModuleRef {
    /// Creates one exact descriptor reference.
    #[must_use]
    pub const fn new(
        id: StandardModuleId,
        semantic_version: StandardModuleSemanticVersion,
        expansion_version: StandardModuleExpansionVersion,
    ) -> Self {
        Self {
            id,
            semantic_version,
            expansion_version,
        }
    }

    /// Returns the standard-module identifier.
    #[must_use]
    pub const fn id(&self) -> &StandardModuleId {
        &self.id
    }
    /// Returns the semantic version.
    #[must_use]
    pub const fn semantic_version(&self) -> StandardModuleSemanticVersion {
        self.semantic_version
    }
    /// Returns the expansion version.
    #[must_use]
    pub const fn expansion_version(&self) -> StandardModuleExpansionVersion {
        self.expansion_version
    }

    /// Returns the current provisional `Exactly` descriptor reference.
    #[must_use]
    pub fn exactly() -> Self {
        Self::new(
            StandardModuleId::exactly(),
            StandardModuleSemanticVersion::one(),
            StandardModuleExpansionVersion::one(),
        )
    }
}

/// Opaque identity of one exact canonical standard-module expansion.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardModuleExpansionFingerprint([u8; 32]);

impl StandardModuleExpansionFingerprint {
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for StandardModuleExpansionFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "StandardModuleExpansionFingerprint(")?;
        fmt_digest(formatter, &self.0)?;
        formatter.write_str(")")
    }
}

impl fmt::Display for StandardModuleExpansionFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_digest(formatter, &self.0)
    }
}

fn fmt_digest(formatter: &mut fmt::Formatter<'_>, bytes: &[u8; 32]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

/// Stable descriptor parameter key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardParameterKey(String);

impl StandardParameterKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
    #[must_use]
    pub fn threshold() -> Self {
        Self("threshold".to_owned())
    }
}

/// Descriptor-defined enumerated parameter value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardEnumValue(String);

impl StandardEnumValue {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Closed dynamic standard-module parameter value family.
pub enum StandardParameterValue<D> {
    LogicLevel(LogicLevel),
    U64(u64),
    Span(Span<D>),
    NonZeroSpan(NonZeroSpan<D>),
    Enum(StandardEnumValue),
}

impl<D> Clone for StandardParameterValue<D> {
    fn clone(&self) -> Self {
        match self {
            Self::LogicLevel(value) => Self::LogicLevel(*value),
            Self::U64(value) => Self::U64(*value),
            Self::Span(value) => Self::Span(*value),
            Self::NonZeroSpan(value) => Self::NonZeroSpan(*value),
            Self::Enum(value) => Self::Enum(value.clone()),
        }
    }
}

impl<D> fmt::Debug for StandardParameterValue<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LogicLevel(value) => formatter.debug_tuple("LogicLevel").field(value).finish(),
            Self::U64(value) => formatter.debug_tuple("U64").field(value).finish(),
            Self::Span(value) => formatter.debug_tuple("Span").field(value).finish(),
            Self::NonZeroSpan(value) => formatter.debug_tuple("NonZeroSpan").field(value).finish(),
            Self::Enum(value) => formatter.debug_tuple("Enum").field(value).finish(),
        }
    }
}

impl<D> PartialEq for StandardParameterValue<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::LogicLevel(a), Self::LogicLevel(b)) => a == b,
            (Self::U64(a), Self::U64(b)) => a == b,
            (Self::Span(a), Self::Span(b)) => a == b,
            (Self::NonZeroSpan(a), Self::NonZeroSpan(b)) => a == b,
            (Self::Enum(a), Self::Enum(b)) => a == b,
            _ => false,
        }
    }
}

impl<D> Eq for StandardParameterValue<D> {}

/// Runtime-independent kind of one dynamic parameter value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StandardParameterKind {
    LogicLevel,
    U64,
    Span,
    NonZeroSpan,
    Enum,
}

impl<D> StandardParameterValue<D> {
    #[must_use]
    pub const fn kind(&self) -> StandardParameterKind {
        match self {
            Self::LogicLevel(_) => StandardParameterKind::LogicLevel,
            Self::U64(_) => StandardParameterKind::U64,
            Self::Span(_) => StandardParameterKind::Span,
            Self::NonZeroSpan(_) => StandardParameterKind::NonZeroSpan,
            Self::Enum(_) => StandardParameterKind::Enum,
        }
    }
}

/// One canonical parameter assignment retained by a declaration.
pub struct StandardParameterAssignment<D> {
    key: StandardParameterKey,
    value: StandardParameterValue<D>,
}

impl<D> Clone for StandardParameterAssignment<D> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

impl<D> fmt::Debug for StandardParameterAssignment<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandardParameterAssignment")
            .field("key", &self.key)
            .field("value", &self.value)
            .finish()
    }
}

impl<D> PartialEq for StandardParameterAssignment<D> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.value == other.value
    }
}

impl<D> Eq for StandardParameterAssignment<D> {}

impl<D> StandardParameterAssignment<D> {
    #[must_use]
    pub fn key(&self) -> &StandardParameterKey {
        &self.key
    }
    #[must_use]
    pub const fn value(&self) -> &StandardParameterValue<D> {
        &self.value
    }
}

/// Category of one generated canonical internal subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StandardInternalCategory {
    Node,
    InputPort,
    OutputPort,
    Connection,
    Export,
}

/// Stable role and derived key of one canonical internal subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardInternalRole {
    category: StandardInternalCategory,
    role: String,
    key: u128,
    public_input: Option<ModuleInputKey<Level>>,
}

impl StandardInternalRole {
    #[must_use]
    pub const fn category(&self) -> StandardInternalCategory {
        self.category
    }
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }
    #[must_use]
    pub const fn key(&self) -> u128 {
        self.key
    }
    #[must_use]
    pub const fn public_input(&self) -> Option<ModuleInputKey<Level>> {
        self.public_input
    }
}

/// Validated exact standard-module declaration retained by [`ModuleDef`].
pub struct StandardModuleDeclaration<D> {
    module_ref: StandardModuleRef,
    parameters: Vec<StandardParameterAssignment<D>>,
    variadic_inputs: Vec<ModuleInputKey<Level>>,
    expansion_fingerprint: StandardModuleExpansionFingerprint,
    internal_roles: Vec<StandardInternalRole>,
}

impl<D> Clone for StandardModuleDeclaration<D> {
    fn clone(&self) -> Self {
        Self {
            module_ref: self.module_ref.clone(),
            parameters: self.parameters.clone(),
            variadic_inputs: self.variadic_inputs.clone(),
            expansion_fingerprint: self.expansion_fingerprint,
            internal_roles: self.internal_roles.clone(),
        }
    }
}

impl<D> fmt::Debug for StandardModuleDeclaration<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandardModuleDeclaration")
            .field("module_ref", &self.module_ref)
            .field("parameters", &self.parameters)
            .field("variadic_inputs", &self.variadic_inputs)
            .field("expansion_fingerprint", &self.expansion_fingerprint)
            .field("internal_roles", &self.internal_roles)
            .finish()
    }
}

impl<D> PartialEq for StandardModuleDeclaration<D> {
    fn eq(&self, other: &Self) -> bool {
        self.module_ref == other.module_ref
            && self.parameters == other.parameters
            && self.variadic_inputs == other.variadic_inputs
            && self.expansion_fingerprint == other.expansion_fingerprint
            && self.internal_roles == other.internal_roles
    }
}

impl<D> Eq for StandardModuleDeclaration<D> {}

impl<D> StandardModuleDeclaration<D> {
    #[must_use]
    pub const fn module_ref(&self) -> &StandardModuleRef {
        &self.module_ref
    }
    #[must_use]
    pub fn parameters(&self) -> impl ExactSizeIterator<Item = &StandardParameterAssignment<D>> {
        self.parameters.iter()
    }
    #[must_use]
    pub fn variadic_inputs(&self) -> impl ExactSizeIterator<Item = ModuleInputKey<Level>> + '_ {
        self.variadic_inputs.iter().copied()
    }
    #[must_use]
    pub const fn expansion_fingerprint(&self) -> StandardModuleExpansionFingerprint {
        self.expansion_fingerprint
    }
    #[must_use]
    pub fn internal_roles(&self) -> impl ExactSizeIterator<Item = &StandardInternalRole> {
        self.internal_roles.iter()
    }

    #[must_use]
    pub fn exactly_threshold(&self) -> Option<u64> {
        if self.module_ref.id.as_str() != EXACTLY_ID {
            return None;
        }
        self.parameters.iter().find_map(|assignment| {
            (assignment.key.as_str() == "threshold")
                .then_some(match assignment.value {
                    StandardParameterValue::U64(value) => Some(value),
                    _ => None,
                })
                .flatten()
        })
    }

    /// Returns the declaration-sensitive dependency class for `Exactly`.
    #[must_use]
    pub fn exactly_dependency(&self) -> Option<ExactlyDependency> {
        let threshold = self.exactly_threshold()?;
        Some(
            if self.variadic_inputs.is_empty() || threshold > self.variadic_inputs.len() as u64 {
                ExactlyDependency::Constant
            } else {
                ExactlyDependency::EveryInput
            },
        )
    }

    /// Returns the public current-reaction dependency relation for this declaration.
    #[must_use]
    pub fn public_dependencies(&self) -> Vec<StandardPublicDependency> {
        if self.exactly_dependency() != Some(ExactlyDependency::EveryInput) {
            return Vec::new();
        }
        self.variadic_inputs
            .iter()
            .copied()
            .map(|input| StandardPublicDependency {
                input: input.into(),
                output: exactly_result_key().into(),
            })
            .collect()
    }

    pub(crate) fn fingerprint_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_text(&mut bytes, self.module_ref.id.as_str());
        bytes.extend_from_slice(&self.module_ref.semantic_version.get().to_be_bytes());
        bytes.extend_from_slice(&self.module_ref.expansion_version.get().to_be_bytes());
        for parameter in &self.parameters {
            push_text(&mut bytes, parameter.key.as_str());
            match &parameter.value {
                StandardParameterValue::LogicLevel(value) => bytes.push(u8::from(value.is_high())),
                StandardParameterValue::U64(value) => bytes.extend_from_slice(&value.to_be_bytes()),
                StandardParameterValue::Span(value) => {
                    bytes.extend_from_slice(&value.ticks().to_be_bytes())
                }
                StandardParameterValue::NonZeroSpan(value) => {
                    bytes.extend_from_slice(&value.ticks().to_be_bytes())
                }
                StandardParameterValue::Enum(value) => push_text(&mut bytes, value.as_str()),
            }
        }
        for input in &self.variadic_inputs {
            bytes.extend_from_slice(&input.as_u128().to_be_bytes());
        }
        bytes.extend_from_slice(&self.expansion_fingerprint.as_bytes());
        push_internal_roles(&mut bytes, &self.internal_roles);
        bytes
    }
}

/// Broad descriptor category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardModuleCategory {
    Combinational,
    Stateful,
    Temporal,
}

/// Availability of one descriptor in this catalogue build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardModuleAvailability {
    Available,
    Deprecated,
}

/// Structured public port schema entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardPortSchema {
    role: &'static str,
    kind: SignalKind,
    variadic: bool,
    fixed_input: Option<AnyModuleInputKey>,
    fixed_output: Option<AnyModuleOutputKey>,
}

impl StandardPortSchema {
    #[must_use]
    pub const fn role(&self) -> &'static str {
        self.role
    }
    #[must_use]
    pub const fn kind(&self) -> SignalKind {
        self.kind
    }
    #[must_use]
    pub const fn is_variadic(&self) -> bool {
        self.variadic
    }
    #[must_use]
    pub const fn fixed_input(&self) -> Option<AnyModuleInputKey> {
        self.fixed_input
    }
    #[must_use]
    pub const fn fixed_output(&self) -> Option<AnyModuleOutputKey> {
        self.fixed_output
    }
}

/// Structured parameter schema entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardParameterSchema {
    key: StandardParameterKey,
    kind: StandardParameterKind,
    required: bool,
}

impl StandardParameterSchema {
    #[must_use]
    pub fn key(&self) -> &StandardParameterKey {
        &self.key
    }
    #[must_use]
    pub const fn kind(&self) -> StandardParameterKind {
        self.kind
    }
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}

/// Immutable descriptor metadata for one standard module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardModuleDescriptor<D> {
    module_ref: StandardModuleRef,
    introduced: StandardCatalogueVersion,
    display_name: &'static str,
    documentation: &'static str,
    category: StandardModuleCategory,
    availability: StandardModuleAvailability,
    inputs: Vec<StandardPortSchema>,
    outputs: Vec<StandardPortSchema>,
    parameters: Vec<StandardParameterSchema>,
    marker: PhantomData<fn() -> D>,
}

impl<D> StandardModuleDescriptor<D> {
    fn exactly() -> Self {
        Self {
            module_ref: StandardModuleRef::exactly(),
            introduced: StandardCatalogueVersion::one(),
            display_name: "Exactly",
            documentation: "High exactly when the number of High inputs equals threshold.",
            category: StandardModuleCategory::Combinational,
            availability: StandardModuleAvailability::Available,
            inputs: vec![StandardPortSchema {
                role: "inputs",
                kind: SignalKind::Level,
                variadic: true,
                fixed_input: None,
                fixed_output: None,
            }],
            outputs: vec![StandardPortSchema {
                role: "result",
                kind: SignalKind::Level,
                variadic: false,
                fixed_input: None,
                fixed_output: Some(exactly_result_key().into()),
            }],
            parameters: vec![StandardParameterSchema {
                key: StandardParameterKey::threshold(),
                kind: StandardParameterKind::U64,
                required: true,
            }],
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn module_ref(&self) -> &StandardModuleRef {
        &self.module_ref
    }
    #[must_use]
    pub const fn introduced_in(&self) -> StandardCatalogueVersion {
        self.introduced
    }
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        self.display_name
    }
    #[must_use]
    pub const fn documentation(&self) -> &'static str {
        self.documentation
    }
    #[must_use]
    pub const fn category(&self) -> StandardModuleCategory {
        self.category
    }
    #[must_use]
    pub const fn availability(&self) -> StandardModuleAvailability {
        self.availability
    }
    #[must_use]
    pub const fn is_stateful(&self) -> bool {
        matches!(self.category, StandardModuleCategory::Stateful)
    }
    #[must_use]
    pub const fn is_temporal(&self) -> bool {
        matches!(self.category, StandardModuleCategory::Temporal)
    }
    #[must_use]
    pub fn inputs(&self) -> &[StandardPortSchema] {
        &self.inputs
    }
    #[must_use]
    pub fn outputs(&self) -> &[StandardPortSchema] {
        &self.outputs
    }
    #[must_use]
    pub fn parameters(&self) -> &[StandardParameterSchema] {
        &self.parameters
    }
}

/// Owned unchecked dynamic request for one exact standard descriptor.
pub struct StandardModuleRequest<D> {
    module_ref: StandardModuleRef,
    parameters: Vec<(StandardParameterKey, StandardParameterValue<D>)>,
    variadic_inputs: Vec<AnyModuleInputKey>,
}

impl<D> StandardModuleRequest<D> {
    #[must_use]
    pub fn new(module_ref: StandardModuleRef) -> Self {
        Self {
            module_ref,
            parameters: Vec::new(),
            variadic_inputs: Vec::new(),
        }
    }
    #[must_use]
    pub fn with_parameter(
        mut self,
        key: StandardParameterKey,
        value: StandardParameterValue<D>,
    ) -> Self {
        self.parameters.push((key, value));
        self
    }
    #[must_use]
    pub fn with_variadic_input(mut self, input: AnyModuleInputKey) -> Self {
        self.variadic_inputs.push(input);
        self
    }
    #[must_use]
    pub const fn module_ref(&self) -> &StandardModuleRef {
        &self.module_ref
    }
}

/// Structured exact-lookup failure.
#[non_exhaustive]
pub enum CatalogueFailure<D> {
    UnknownId(Problem<D>),
    UnsupportedVersion(Problem<D>),
}

impl<D> CatalogueFailure<D> {
    #[must_use]
    pub const fn problem(&self) -> &Problem<D> {
        match self {
            Self::UnknownId(problem) | Self::UnsupportedVersion(problem) => problem,
        }
    }
    fn into_problem(self) -> Problem<D> {
        match self {
            Self::UnknownId(problem) | Self::UnsupportedVersion(problem) => problem,
        }
    }
}

impl<D> fmt::Debug for CatalogueFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct(match self {
                Self::UnknownId(_) => "UnknownId",
                Self::UnsupportedVersion(_) => "UnsupportedVersion",
            })
            .field("code", &self.problem().code())
            .finish()
    }
}

/// Immutable built-in catalogue view.
pub struct StandardCatalogue<D> {
    descriptors: Vec<StandardModuleDescriptor<D>>,
}

impl<D> Default for StandardCatalogue<D> {
    fn default() -> Self {
        Self::current()
    }
}

impl<D> StandardCatalogue<D> {
    #[must_use]
    pub fn current() -> Self {
        Self {
            descriptors: vec![StandardModuleDescriptor::exactly()],
        }
    }
    #[must_use]
    pub const fn version(&self) -> StandardCatalogueVersion {
        StandardCatalogueVersion::one()
    }
    #[must_use]
    pub fn descriptors(&self) -> impl ExactSizeIterator<Item = &StandardModuleDescriptor<D>> {
        self.descriptors.iter()
    }

    #[allow(clippy::result_large_err)]
    pub fn descriptor(
        &self,
        module_ref: &StandardModuleRef,
    ) -> Result<&StandardModuleDescriptor<D>, CatalogueFailure<D>> {
        if let Some(found) = self
            .descriptors
            .iter()
            .find(|descriptor| descriptor.module_ref() == module_ref)
        {
            return Ok(found);
        }
        let problem = if self
            .descriptors
            .iter()
            .any(|descriptor| descriptor.module_ref.id == module_ref.id)
        {
            Problem::new(
                SubjectRef::StandardCatalogue,
                Vec::new(),
                ProblemEvidence::standard_module_unsupported_version(module_ref.clone()),
            )
        } else {
            Problem::new(
                SubjectRef::StandardCatalogue,
                Vec::new(),
                ProblemEvidence::standard_module_unknown_id(module_ref.clone()),
            )
        };
        if self
            .descriptors
            .iter()
            .any(|descriptor| descriptor.module_ref.id == module_ref.id)
        {
            Err(CatalogueFailure::UnsupportedVersion(problem))
        } else {
            Err(CatalogueFailure::UnknownId(problem))
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn latest(
        &self,
        id: &StandardModuleId,
    ) -> Result<&StandardModuleDescriptor<D>, CatalogueFailure<D>> {
        if let Some(found) = self
            .descriptors
            .iter()
            .rev()
            .find(|descriptor| descriptor.module_ref.id() == id)
        {
            return Ok(found);
        }
        let requested = StandardModuleRef::new(
            id.clone(),
            StandardModuleSemanticVersion::one(),
            StandardModuleExpansionVersion::one(),
        );
        Err(CatalogueFailure::UnknownId(Problem::new(
            SubjectRef::StandardCatalogue,
            Vec::new(),
            ProblemEvidence::standard_module_unknown_id(requested),
        )))
    }

    #[must_use]
    pub fn build(&self, request: StandardModuleRequest<D>) -> Report<ModuleDef<D>, D>
    where
        D: PartialEq,
    {
        let mut diagnostics = DiagnosticSet::new();
        if let Err(failure) = self.descriptor(&request.module_ref) {
            insert_problem(&mut diagnostics, failure.into_problem());
            return Report::new(None, diagnostics);
        }
        let threshold_key = StandardParameterKey::threshold();
        let mut threshold = None;
        for (key, value) in &request.parameters {
            if key != &threshold_key {
                insert_evidence(
                    &mut diagnostics,
                    ProblemEvidence::standard_module_unexpected_parameter(
                        request.module_ref.clone(),
                        key.clone(),
                    ),
                );
            } else if value.kind() != StandardParameterKind::U64 {
                insert_evidence(
                    &mut diagnostics,
                    ProblemEvidence::standard_module_parameter_kind_mismatch(
                        request.module_ref.clone(),
                        key.clone(),
                        StandardParameterKind::U64,
                        value.kind(),
                    ),
                );
            } else if threshold.is_some() {
                insert_evidence(
                    &mut diagnostics,
                    ProblemEvidence::standard_module_unexpected_parameter(
                        request.module_ref.clone(),
                        key.clone(),
                    ),
                );
            } else if let StandardParameterValue::U64(value) = value {
                threshold = Some(*value);
            }
        }
        if threshold.is_none() {
            insert_evidence(
                &mut diagnostics,
                ProblemEvidence::standard_module_missing_parameter(
                    request.module_ref.clone(),
                    threshold_key,
                ),
            );
        }
        let mut inputs = Vec::new();
        let mut seen = BTreeSet::new();
        for input in &request.variadic_inputs {
            match input {
                AnyModuleInputKey::Level(key) if seen.insert(*key) => inputs.push(*key),
                _ => insert_evidence(
                    &mut diagnostics,
                    ProblemEvidence::standard_module_interface_mismatch(
                        request.module_ref.clone(),
                        request.variadic_inputs.clone(),
                    ),
                ),
            }
        }
        if diagnostics.has_severity(crate::diagnostics::Severity::Error) {
            return Report::new(None, diagnostics);
        }
        let Some(threshold) = threshold else {
            return Report::new(None, diagnostics);
        };
        inputs.sort();
        let (unchecked, roles) = exactly_expansion::<D>(&request.module_ref, threshold, &inputs);
        let mut generated = BTreeSet::new();
        for role in &roles {
            if !generated.insert((role.category(), role.key())) {
                insert_evidence(
                    &mut diagnostics,
                    ProblemEvidence::standard_module_internal_key_collision(
                        request.module_ref.clone(),
                        role.key(),
                    ),
                );
            }
        }
        if diagnostics.has_severity(crate::diagnostics::Severity::Error) {
            return Report::new(None, diagnostics);
        }
        let (artifact, validation) = unchecked.validate().into_parts();
        for finding in validation {
            diagnostics.insert(finding);
        }
        let Some(user_module) = artifact else {
            return Report::new(None, diagnostics);
        };
        let expansion_fingerprint = expansion_fingerprint(
            user_module.definition(),
            &request.module_ref,
            threshold,
            &inputs,
            &roles,
        );
        let declaration = StandardModuleDeclaration {
            module_ref: request.module_ref.clone(),
            parameters: vec![StandardParameterAssignment {
                key: StandardParameterKey::threshold(),
                value: StandardParameterValue::U64(threshold),
            }],
            variadic_inputs: inputs.clone(),
            expansion_fingerprint,
            internal_roles: roles,
        };
        let module = user_module.with_standard_origin(declaration);
        add_exactly_warnings(&mut diagnostics, &module, threshold, &inputs);
        Report::new(Some(module), diagnostics)
    }
}

/// Returns the fixed public result key for `Exactly`.
#[must_use]
pub fn exactly_result_key() -> ModuleOutputKey<Level> {
    ModuleOutputKey::from_u128(derive_public_key(
        PUBLIC_KEY_DOMAIN,
        StandardModuleRef::exactly().id(),
        "output",
        "level",
        "result",
    ))
}

fn add_exactly_warnings<D: PartialEq>(
    diagnostics: &mut DiagnosticSet<D>,
    module: &ModuleDef<D>,
    threshold: u64,
    inputs: &[ModuleInputKey<Level>],
) {
    let module_ref = StandardModuleRef::exactly();
    if inputs.is_empty() {
        insert_for_module(
            diagnostics,
            module,
            ProblemEvidence::standard_module_empty_variadic(module_ref.clone(), inputs.to_vec()),
        );
    }
    if inputs.len() == 1 {
        insert_for_module(
            diagnostics,
            module,
            ProblemEvidence::standard_module_unary_degenerate(module_ref.clone(), inputs.to_vec()),
        );
    }
    if threshold > inputs.len() as u64 {
        insert_for_module(
            diagnostics,
            module,
            ProblemEvidence::standard_module_impossible_threshold(
                module_ref,
                inputs.len(),
                threshold,
            ),
        );
    }
}

fn insert_for_module<D: PartialEq>(
    diagnostics: &mut DiagnosticSet<D>,
    module: &ModuleDef<D>,
    evidence: ProblemEvidence<D>,
) {
    let problem = Problem::new(
        SubjectRef::ModuleDefinition(module.fingerprint()),
        Vec::new(),
        evidence,
    );
    insert_problem(diagnostics, problem);
}

fn insert_evidence<D: PartialEq>(diagnostics: &mut DiagnosticSet<D>, evidence: ProblemEvidence<D>) {
    insert_problem(
        diagnostics,
        Problem::new(SubjectRef::StandardCatalogue, Vec::new(), evidence),
    );
}

fn insert_problem<D: PartialEq>(diagnostics: &mut DiagnosticSet<D>, problem: Problem<D>) {
    if let Ok(diagnostic) = Diagnostic::new(problem) {
        diagnostics.insert(diagnostic);
    }
}

#[derive(Clone, Copy)]
enum ExpansionSource {
    Public(ModuleInputKey<Level>),
    Node(OutPortKey<Level>),
}

struct Expansion<D> {
    module_ref: StandardModuleRef,
    nodes: Vec<NodeDef<D>>,
    connections: Vec<ConnectionDef>,
    mappings: Vec<ModuleInterfaceMapping>,
    roles: Vec<StandardInternalRole>,
}

impl<D> Expansion<D> {
    fn new(module_ref: &StandardModuleRef) -> Self {
        Self {
            module_ref: module_ref.clone(),
            nodes: Vec::new(),
            connections: Vec::new(),
            mappings: Vec::new(),
            roles: Vec::new(),
        }
    }

    fn key(&self, category: &str, role: &str, qualifier: Option<u128>) -> u128 {
        derive_internal_key(
            INTERNAL_KEY_DOMAIN,
            self.module_ref.id(),
            category,
            "level",
            role,
            qualifier,
        )
    }

    fn record(
        &mut self,
        category: StandardInternalCategory,
        role: &str,
        key: u128,
        qualifier: Option<ModuleInputKey<Level>>,
    ) {
        self.roles.push(StandardInternalRole {
            category,
            role: role.to_owned(),
            key,
            public_input: qualifier,
        });
    }

    fn constant(&mut self, role: &str, value: LogicLevel) -> ExpansionSource {
        let node = NodeKey::from_u128(self.key("node", role, None));
        let output_role = format!("{role}.result");
        let output = OutPortKey::from_u128(self.key("output_port", &output_role, None));
        self.record(StandardInternalCategory::Node, role, node.as_u128(), None);
        self.record(
            StandardInternalCategory::OutputPort,
            &output_role,
            output.as_u128(),
            None,
        );
        self.nodes.push(NodeDef::new(
            node,
            NodeKind::constant(value),
            NodePorts::new(Vec::new(), vec![output.into()]),
            DiagnosticMeta::default(),
        ));
        ExpansionSource::Node(output)
    }

    fn not(&mut self, role: &str, source: ExpansionSource) -> ExpansionSource {
        let node = NodeKey::from_u128(self.key("node", role, None));
        let input_role = format!("{role}.input");
        let output_role = format!("{role}.result");
        let input =
            InPortKey::from_u128(self.key("input_port", &input_role, Some(source.qualifier())));
        let output = OutPortKey::from_u128(self.key("output_port", &output_role, None));
        self.record(StandardInternalCategory::Node, role, node.as_u128(), None);
        self.record(
            StandardInternalCategory::InputPort,
            &input_role,
            input.as_u128(),
            source.public_key(),
        );
        self.record(
            StandardInternalCategory::OutputPort,
            &output_role,
            output.as_u128(),
            None,
        );
        self.nodes.push(NodeDef::new(
            node,
            NodeKind::not(),
            NodePorts::new(vec![input.into()], vec![output.into()]),
            DiagnosticMeta::default(),
        ));
        self.connect(source, input, &input_role);
        ExpansionSource::Node(output)
    }

    fn variadic(
        &mut self,
        role: &str,
        kind: NodeKind<D>,
        sources: &[ExpansionSource],
    ) -> ExpansionSource {
        let node = NodeKey::from_u128(self.key("node", role, None));
        let output_role = format!("{role}.result");
        let output = OutPortKey::from_u128(self.key("output_port", &output_role, None));
        let mut inputs = Vec::new();
        self.record(StandardInternalCategory::Node, role, node.as_u128(), None);
        self.record(
            StandardInternalCategory::OutputPort,
            &output_role,
            output.as_u128(),
            None,
        );
        for source in sources.iter().copied() {
            let input_role = format!("{role}.input");
            let input =
                InPortKey::from_u128(self.key("input_port", &input_role, Some(source.qualifier())));
            self.record(
                StandardInternalCategory::InputPort,
                &input_role,
                input.as_u128(),
                source.public_key(),
            );
            inputs.push(input.into());
            self.connect(source, input, &input_role);
        }
        self.nodes.push(NodeDef::new(
            node,
            kind,
            NodePorts::new(inputs, vec![output.into()]),
            DiagnosticMeta::default(),
        ));
        ExpansionSource::Node(output)
    }

    fn connect(&mut self, source: ExpansionSource, target: InPortKey<Level>, role: &str) {
        match source {
            ExpansionSource::Public(input) => self.mappings.push(ModuleInterfaceMapping::input(
                input.into(),
                ConnectionEndpoint::node_input(target.into()),
            )),
            ExpansionSource::Node(output) => {
                let public_input = source.public_key();
                let key = ConnectionKey::from_u128(self.key(
                    "connection",
                    role,
                    Some(source.qualifier()),
                ));
                self.record(
                    StandardInternalCategory::Connection,
                    role,
                    key.as_u128(),
                    public_input,
                );
                self.connections.push(ConnectionDef::new(
                    key,
                    ConnectionEndpoint::node_output(output.into()),
                    ConnectionEndpoint::node_input(target.into()),
                    DiagnosticMeta::default(),
                ));
            }
        }
    }
}

impl ExpansionSource {
    fn public_key(self) -> Option<ModuleInputKey<Level>> {
        match self {
            Self::Public(key) => Some(key),
            Self::Node(_) => None,
        }
    }
    fn qualifier(self) -> u128 {
        match self {
            Self::Public(key) => key.as_u128(),
            Self::Node(key) => key.as_u128(),
        }
    }
    fn endpoint(self) -> ConnectionEndpoint {
        match self {
            Self::Public(key) => ConnectionEndpoint::module_input(key.into()),
            Self::Node(key) => ConnectionEndpoint::node_output(key.into()),
        }
    }
}

fn exactly_expansion<D>(
    module_ref: &StandardModuleRef,
    threshold: u64,
    inputs: &[ModuleInputKey<Level>],
) -> (UncheckedModule<D>, Vec<StandardInternalRole>) {
    // SPEC: docs/specs/contracts/standard-exactly.yaml "fixed-canonical-case-expansion"
    // Identity follows the specified case table, never a behaviorally equivalent graph.
    let mut expansion = Expansion::new(module_ref);
    let sources: Vec<_> = inputs
        .iter()
        .copied()
        .map(ExpansionSource::Public)
        .collect();
    let arity = inputs.len() as u64;
    let result = if threshold > arity {
        expansion.constant("constant_result", LogicLevel::Low)
    } else if threshold == 0 && arity == 0 {
        expansion.constant("constant_result", LogicLevel::High)
    } else if threshold == 0 && arity == 1 {
        expansion.not("not_only", sole_source(&sources))
    } else if threshold == 0 {
        let any = expansion.variadic("any_input", NodeKind::any(), &sources);
        expansion.not("not_any", any)
    } else if threshold == arity && arity == 1 {
        sole_source(&sources)
    } else if threshold == arity {
        expansion.variadic("all_input", NodeKind::all(), &sources)
    } else {
        let lower = expansion.variadic("at_least_lower", NodeKind::at_least(threshold), &sources);
        let upper = expansion.variadic(
            "at_least_upper",
            NodeKind::at_least(threshold + 1),
            &sources,
        );
        let not_upper = expansion.not("not_upper", upper);
        expansion.variadic("combine", NodeKind::all(), &[lower, not_upper])
    };
    let output = exactly_result_key();
    let export_key = expansion.key("export", "result", Some(result.qualifier()));
    expansion.roles.push(StandardInternalRole {
        category: StandardInternalCategory::Export,
        role: "result".to_owned(),
        key: export_key,
        public_input: result.public_key(),
    });
    expansion.mappings.push(ModuleInterfaceMapping::output(
        output.into(),
        result.endpoint(),
    ));
    let module_inputs = inputs
        .iter()
        .copied()
        .map(|key| ModuleInputDef::new(key.into(), DiagnosticMeta::default()))
        .collect();
    let module_outputs = vec![ModuleOutputDef::new(
        output.into(),
        DiagnosticMeta::default(),
    )];
    expansion
        .roles
        .sort_by(|a, b| (a.category, &a.role, a.key).cmp(&(b.category, &b.role, b.key)));
    let module = UncheckedModule::new_user(
        DiagnosticMeta::default(),
        module_inputs,
        module_outputs,
        expansion.mappings,
        expansion.nodes,
        expansion.connections,
    );
    (module, expansion.roles)
}

fn sole_source(sources: &[ExpansionSource]) -> ExpansionSource {
    match sources {
        [source] => *source,
        _ => panic!("Exactly unary canonical branch requires exactly one public source"),
    }
}

fn expansion_fingerprint<D>(
    module: &UncheckedModule<D>,
    module_ref: &StandardModuleRef,
    threshold: u64,
    inputs: &[ModuleInputKey<Level>],
    roles: &[StandardInternalRole],
) -> StandardModuleExpansionFingerprint {
    let mut bytes = Vec::new();
    push_text(&mut bytes, EXPANSION_FINGERPRINT_DOMAIN);
    push_text(&mut bytes, module_ref.id.as_str());
    bytes.extend_from_slice(&module_ref.semantic_version.get().to_be_bytes());
    bytes.extend_from_slice(&module_ref.expansion_version.get().to_be_bytes());
    bytes.extend_from_slice(&threshold.to_be_bytes());
    for input in inputs {
        bytes.extend_from_slice(&input.as_u128().to_be_bytes());
    }
    push_internal_roles(&mut bytes, roles);
    bytes.extend_from_slice(&crate::identity::module_canonical_bytes(module));
    StandardModuleExpansionFingerprint::from_bytes(*blake3::hash(&bytes).as_bytes())
}

fn push_internal_roles(bytes: &mut Vec<u8>, roles: &[StandardInternalRole]) {
    bytes.extend_from_slice(&(roles.len() as u64).to_be_bytes());
    for role in roles {
        bytes.push(match role.category {
            StandardInternalCategory::Node => 0,
            StandardInternalCategory::InputPort => 1,
            StandardInternalCategory::OutputPort => 2,
            StandardInternalCategory::Connection => 3,
            StandardInternalCategory::Export => 4,
        });
        push_text(bytes, &role.role);
        bytes.extend_from_slice(&role.key.to_be_bytes());
        match role.public_input {
            Some(input) => {
                bytes.push(1);
                bytes.extend_from_slice(&input.as_u128().to_be_bytes());
            }
            None => bytes.push(0),
        }
    }
}

fn derive_public_key(
    domain: &str,
    module_id: &StandardModuleId,
    direction: &str,
    signal_kind: &str,
    role: &str,
) -> u128 {
    let mut bytes = Vec::new();
    push_text(&mut bytes, domain);
    push_text(&mut bytes, module_id.as_str());
    push_text(&mut bytes, direction);
    push_text(&mut bytes, signal_kind);
    push_text(&mut bytes, role);
    truncated_key(&bytes)
}

fn derive_internal_key(
    domain: &str,
    module_id: &StandardModuleId,
    category: &str,
    signal_kind: &str,
    role: &str,
    qualifier: Option<u128>,
) -> u128 {
    let mut bytes = Vec::new();
    push_text(&mut bytes, domain);
    push_text(&mut bytes, module_id.as_str());
    push_text(&mut bytes, category);
    push_text(&mut bytes, signal_kind);
    push_text(&mut bytes, role);
    if let Some(qualifier) = qualifier {
        bytes.extend_from_slice(&qualifier.to_be_bytes());
    }
    truncated_key(&bytes)
}

fn truncated_key(bytes: &[u8]) -> u128 {
    let digest = blake3::hash(bytes);
    let mut key = [0; 16];
    key.copy_from_slice(&digest.as_bytes()[..16]);
    u128::from_be_bytes(key)
}

fn push_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

/// Declaration-sensitive public dependency classification for `Exactly`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactlyDependency {
    Constant,
    EveryInput,
}

/// One conservative current-reaction dependency across a standard-module boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardPublicDependency {
    input: AnyModuleInputKey,
    output: AnyModuleOutputKey,
}

impl StandardPublicDependency {
    #[must_use]
    pub const fn input(self) -> AnyModuleInputKey {
        self.input
    }

    #[must_use]
    pub const fn output(self) -> AnyModuleOutputKey {
        self.output
    }
}

/// Structured public summary of one initialized `Exactly` instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactlyInspection {
    threshold: u64,
    arity: usize,
    high_count: usize,
    low_count: usize,
    high_inputs: Vec<ModuleInputKey<Level>>,
    low_inputs: Vec<ModuleInputKey<Level>>,
    result: LogicLevel,
    dependency: ExactlyDependency,
}

impl ExactlyInspection {
    pub(crate) fn new(
        threshold: u64,
        levels: impl Iterator<Item = (ModuleInputKey<Level>, LogicLevel)>,
        result: LogicLevel,
    ) -> Self {
        let levels: Vec<_> = levels.collect();
        let high_inputs = levels
            .iter()
            .filter_map(|(key, level)| level.is_high().then_some(*key))
            .collect::<Vec<_>>();
        let low_inputs = levels
            .iter()
            .filter_map(|(key, level)| (!level.is_high()).then_some(*key))
            .collect::<Vec<_>>();
        let high_count = high_inputs.len();
        let arity = levels.len();
        Self {
            threshold,
            arity,
            high_count,
            low_count: arity - high_count,
            high_inputs,
            low_inputs,
            result,
            dependency: if arity == 0 || threshold > arity as u64 {
                ExactlyDependency::Constant
            } else {
                ExactlyDependency::EveryInput
            },
        }
    }
    #[must_use]
    pub const fn threshold(&self) -> u64 {
        self.threshold
    }
    #[must_use]
    pub const fn arity(&self) -> usize {
        self.arity
    }
    #[must_use]
    pub const fn high_count(&self) -> usize {
        self.high_count
    }
    #[must_use]
    pub const fn low_count(&self) -> usize {
        self.low_count
    }
    #[must_use]
    pub fn high_inputs(&self) -> &[ModuleInputKey<Level>] {
        &self.high_inputs
    }
    #[must_use]
    pub fn low_inputs(&self) -> &[ModuleInputKey<Level>] {
        &self.low_inputs
    }
    #[must_use]
    pub const fn result(&self) -> LogicLevel {
        self.result
    }
    #[must_use]
    pub const fn dependency(&self) -> ExactlyDependency {
        self.dependency
    }
    #[must_use]
    pub const fn constant_result(&self) -> bool {
        matches!(self.dependency, ExactlyDependency::Constant)
    }
    #[must_use]
    pub fn explanation(&self) -> ExactlyExplanation {
        if self.threshold > self.arity as u64 {
            ExactlyExplanation::Impossible {
                threshold: self.threshold,
                arity: self.arity,
            }
        } else if self.result.is_high() {
            ExactlyExplanation::Matched {
                high_contributors: self.high_inputs.clone(),
                low_non_contributors: self.low_inputs.clone(),
            }
        } else if self.high_count < self.threshold as usize {
            ExactlyExplanation::Deficit {
                missing: self.threshold as usize - self.high_count,
                low_inputs: self.low_inputs.clone(),
            }
        } else {
            ExactlyExplanation::Excess {
                excess: self.high_count - self.threshold as usize,
                high_inputs: self.high_inputs.clone(),
            }
        }
    }
}

/// Structured public reason for an `Exactly` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactlyExplanation {
    Matched {
        high_contributors: Vec<ModuleInputKey<Level>>,
        low_non_contributors: Vec<ModuleInputKey<Level>>,
    },
    Deficit {
        missing: usize,
        low_inputs: Vec<ModuleInputKey<Level>>,
    },
    Excess {
        excess: usize,
        high_inputs: Vec<ModuleInputKey<Level>>,
    },
    Impossible {
        threshold: u64,
        arity: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_module_fingerprint_is_sensitive_to_internal_roles() {
        let input = ModuleInputKey::<Level>::from_u128(1);
        let request = StandardModuleRequest::new(StandardModuleRef::exactly())
            .with_parameter(
                StandardParameterKey::threshold(),
                StandardParameterValue::U64(1),
            )
            .with_variadic_input(input.into());
        let module = StandardCatalogue::<()>::current()
            .build(request)
            .require_artifact()
            .unwrap();
        let mut declaration = module.standard_declaration().unwrap().clone();
        let original_expansion = declaration.expansion_fingerprint;
        declaration.internal_roles[0].role.push_str(".changed");
        let changed_expansion = expansion_fingerprint(
            module.definition(),
            &declaration.module_ref,
            1,
            &[input],
            &declaration.internal_roles,
        );
        let changed = module.clone().with_standard_origin(declaration);

        assert_ne!(original_expansion, changed_expansion);
        assert_ne!(module.fingerprint(), changed.fingerprint());
    }
}
