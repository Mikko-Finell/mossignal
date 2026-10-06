//! Structural preparation of one topology replacement.
//!
//! A patch is an owned declarative rewrite. Preparation validates and compiles
//! the target through the ordinary network path and builds a state-independent
//! migration plan. It does not read or mutate a running machine.

#![allow(clippy::result_large_err)]

use crate::authored::{
    ConnectionDef, ConnectionEndpoint, EdgeDetectorKind, ExternalInputDef, ExternalOutputDef,
    ModuleInstanceDef, NodeDef, NodeKind, UncheckedNetwork,
};
use crate::compile::CompiledNetwork;
use crate::diagnostics::{
    Diagnostic, DiagnosticSet, Problem, ProblemEvidence, RelatedSubject, RelatedSubjectRole,
    Report, SubjectRef,
};
use crate::identity::{InputSchemaFingerprint, NetworkFingerprint, TimeDomainId};
use crate::key::{
    AnyExternalInputKey, AnyExternalOutputKey, AnyInPortKey, AnyOutPortKey, AnySignalSourceKey,
    ConnectionKey, ModuleInstanceKey, NetworkKey, NodeKey, SignalSourceKey,
};
use crate::machine::NetworkRevision;
use crate::metadata::DiagnosticMeta;
use crate::module::{ModuleDef, QualifiedNodeRef};
use crate::node_schema::{StateFamily, TemporalFamily, node_schema};
use crate::signal::SignalKind;
use crate::standard::{
    StandardInternalCategory, StandardModuleDeclaration, StandardParameterAssignment,
};
use core::fmt;
use core::marker::PhantomData;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Closed overdue policy for a recomputed deadline that falls before patch time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OverdueMigrationPolicy {
    /// Finalization rejects when the recomputed deadline is already past.
    Reject,
    /// The surviving obligation becomes due at patch time.
    MatureAtPatchTime,
}

/// Explicit pending-work policy for a preserved [`NodeKind::PulseDelay`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PulseDelayMigration {
    /// Keep existing deadlines and apply the target delay only to later input.
    PreserveDeadlines,
    /// Rebuild each deadline from its origin plus the target delay.
    RecomputeFromOrigin {
        /// Policy used when the rebuilt deadline is already due.
        overdue: OverdueMigrationPolicy,
    },
    /// Restart every pending group from patch time.
    RestartFromPatchTime,
    /// Cancel every actual pending group.
    CancelPending,
    /// Reject finalization when any pending group exists.
    RejectIfPending,
}

/// Explicit pending-work policy for a preserved [`NodeKind::TransportDelay`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TransportDelayMigration {
    /// Keep queued transition deadlines.
    PreserveDeadlines,
    /// Rebuild each deadline from its origin plus the target delay.
    RecomputeFromOrigin {
        /// Policy used when the rebuilt deadline is already due.
        overdue: OverdueMigrationPolicy,
    },
    /// Restart every queued transition from patch time.
    RestartFromPatchTime,
    /// Cancel every queued transition.
    CancelPending,
    /// Reject finalization when the queue is nonempty.
    RejectIfPending,
}

/// Explicit candidate policy for a preserved [`NodeKind::InertialDelay`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InertialDelayMigration {
    /// Keep the candidate deadline.
    PreserveDeadline,
    /// Rebuild the candidate deadline from its qualification origin.
    RecomputeFromOrigin {
        /// Policy used when the rebuilt deadline is already due.
        overdue: OverdueMigrationPolicy,
    },
    /// Restart qualification at patch time.
    RestartFromPatchTime,
    /// Cancel the candidate.
    CancelCandidate,
    /// Reject finalization when a candidate exists.
    RejectIfCandidate,
}

/// Explicit schedule policy for a preserved [`NodeKind::Periodic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PeriodicMigration {
    /// Keep the next eligible deadline.
    PreserveNextDeadline,
    /// Rebuild the next boundary from the existing anchor.
    RecomputeFromExistingAnchor,
    /// Discard the anchor and start a new one at patch time.
    ReanchorAtPatchTime,
    /// Clear the anchor and the pending boundary.
    CancelSchedule,
    /// Reject finalization when an anchor or boundary exists.
    RejectIfAnchored,
}

/// Closed per-node migration directive carried by a replacement or reassociation.
#[non_exhaustive]
pub enum NodeMigrationDirective<D> {
    /// The built-in rule for the source and target kinds.
    Standard,
    /// Finalization must observe lossless preservation.
    RequirePreserve,
    /// Install the target declared initial state and report source loss.
    Reset,
    /// Carry a stored level across boolean-state kinds.
    TransferStoredLevel,
    /// An explicit PulseDelay pending-work rule.
    PulseDelay(PulseDelayMigration),
    /// An explicit TransportDelay pending-work rule.
    TransportDelay(TransportDelayMigration),
    /// An explicit InertialDelay candidate rule.
    InertialDelay(InertialDelayMigration),
    /// An explicit Periodic schedule rule.
    Periodic(PeriodicMigration),
    #[doc(hidden)]
    __Domain(PhantomData<fn() -> D>, core::convert::Infallible),
}

impl<D> Clone for NodeMigrationDirective<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for NodeMigrationDirective<D> {}

impl<D> PartialEq for NodeMigrationDirective<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Standard, Self::Standard)
            | (Self::RequirePreserve, Self::RequirePreserve)
            | (Self::Reset, Self::Reset)
            | (Self::TransferStoredLevel, Self::TransferStoredLevel) => true,
            (Self::PulseDelay(left), Self::PulseDelay(right)) => left == right,
            (Self::TransportDelay(left), Self::TransportDelay(right)) => left == right,
            (Self::InertialDelay(left), Self::InertialDelay(right)) => left == right,
            (Self::Periodic(left), Self::Periodic(right)) => left == right,
            (Self::__Domain(_, never), Self::__Domain(_, other)) => match (*never, *other) {},
            _ => false,
        }
    }
}

impl<D> Eq for NodeMigrationDirective<D> {}

impl<D> fmt::Debug for NodeMigrationDirective<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard => formatter.write_str("Standard"),
            Self::RequirePreserve => formatter.write_str("RequirePreserve"),
            Self::Reset => formatter.write_str("Reset"),
            Self::TransferStoredLevel => formatter.write_str("TransferStoredLevel"),
            Self::PulseDelay(policy) => formatter.debug_tuple("PulseDelay").field(policy).finish(),
            Self::TransportDelay(policy) => formatter
                .debug_tuple("TransportDelay")
                .field(policy)
                .finish(),
            Self::InertialDelay(policy) => formatter
                .debug_tuple("InertialDelay")
                .field(policy)
                .finish(),
            Self::Periodic(policy) => formatter.debug_tuple("Periodic").field(policy).finish(),
            Self::__Domain(_, never) => match *never {},
        }
    }
}

/// One internal-node directive inside an explicit module migration.
#[derive(Eq)]
pub struct ModuleNodeMigrationDirective<D> {
    node: NodeKey,
    migration: NodeMigrationDirective<D>,
}

impl<D> ModuleNodeMigrationDirective<D> {
    /// Pairs one module-local node with a closed node directive.
    #[must_use]
    pub const fn new(node: NodeKey, migration: NodeMigrationDirective<D>) -> Self {
        Self { node, migration }
    }

    /// Returns the module-local node key.
    #[must_use]
    pub const fn node(&self) -> NodeKey {
        self.node
    }

    /// Returns the directive applied to that node.
    #[must_use]
    pub const fn migration(&self) -> NodeMigrationDirective<D> {
        self.migration
    }
}

impl<D> Clone for ModuleNodeMigrationDirective<D> {
    fn clone(&self) -> Self {
        Self {
            node: self.node,
            migration: self.migration,
        }
    }
}

impl<D> PartialEq for ModuleNodeMigrationDirective<D> {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node && self.migration == other.migration
    }
}

impl<D> fmt::Debug for ModuleNodeMigrationDirective<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleNodeMigrationDirective")
            .field("node", &self.node)
            .field("migration", &self.migration)
            .finish()
    }
}

/// Explicit correspondence between two module-local nodes.
#[derive(Eq)]
pub struct ModuleInternalReassociation<D> {
    from: NodeKey,
    to: NodeKey,
    migration: NodeMigrationDirective<D>,
}

impl<D> ModuleInternalReassociation<D> {
    /// Associates one removed internal node with one new internal node.
    #[must_use]
    pub const fn new(from: NodeKey, to: NodeKey, migration: NodeMigrationDirective<D>) -> Self {
        Self {
            from,
            to,
            migration,
        }
    }

    /// Returns the base internal node.
    #[must_use]
    pub const fn from(&self) -> NodeKey {
        self.from
    }

    /// Returns the target internal node.
    #[must_use]
    pub const fn to(&self) -> NodeKey {
        self.to
    }

    /// Returns the directive applied across the pair.
    #[must_use]
    pub const fn migration(&self) -> NodeMigrationDirective<D> {
        self.migration
    }
}

impl<D> Clone for ModuleInternalReassociation<D> {
    fn clone(&self) -> Self {
        Self {
            from: self.from,
            to: self.to,
            migration: self.migration,
        }
    }
}

impl<D> PartialEq for ModuleInternalReassociation<D> {
    fn eq(&self, other: &Self) -> bool {
        self.from == other.from && self.to == other.to && self.migration == other.migration
    }
}

impl<D> fmt::Debug for ModuleInternalReassociation<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleInternalReassociation")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("migration", &self.migration)
            .finish()
    }
}

/// Closed module-instance migration directive.
#[non_exhaustive]
pub enum ModuleMigrationDirective<D> {
    /// Catalogue or stable-key rules, with no caller overrides.
    Standard,
    /// Caller-selected internal overrides and reassociations.
    Explicit {
        /// Directives for internal nodes that keep their keys.
        node_overrides: Vec<ModuleNodeMigrationDirective<D>>,
        /// Explicit internal key changes.
        internal_reassociations: Vec<ModuleInternalReassociation<D>>,
    },
    #[doc(hidden)]
    __Domain(PhantomData<fn() -> D>, core::convert::Infallible),
}

impl<D> Clone for ModuleMigrationDirective<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Standard => Self::Standard,
            Self::Explicit {
                node_overrides,
                internal_reassociations,
            } => Self::Explicit {
                node_overrides: node_overrides.clone(),
                internal_reassociations: internal_reassociations.clone(),
            },
            Self::__Domain(_, never) => match *never {},
        }
    }
}

impl<D> PartialEq for ModuleMigrationDirective<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Standard, Self::Standard) => true,
            (
                Self::Explicit {
                    node_overrides: left_nodes,
                    internal_reassociations: left_links,
                },
                Self::Explicit {
                    node_overrides: right_nodes,
                    internal_reassociations: right_links,
                },
            ) => left_nodes == right_nodes && left_links == right_links,
            (Self::__Domain(_, never), Self::__Domain(_, other)) => match (*never, *other) {},
            _ => false,
        }
    }
}

impl<D> Eq for ModuleMigrationDirective<D> {}

impl<D> fmt::Debug for ModuleMigrationDirective<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard => formatter.write_str("Standard"),
            Self::Explicit {
                node_overrides,
                internal_reassociations,
            } => formatter
                .debug_struct("Explicit")
                .field("node_overrides", node_overrides)
                .field("internal_reassociations", internal_reassociations)
                .finish(),
            Self::__Domain(_, never) => match *never {},
        }
    }
}

/// A structural subject that can receive diagnostic metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum StructuralSubjectRef {
    /// The network carrying the patch.
    Network(NetworkKey),
    /// One module instance.
    Module(ModuleInstanceKey),
    /// One node.
    Node(NodeKey),
    /// One node input port.
    InPort(AnyInPortKey),
    /// One node output port.
    OutPort(AnyOutPortKey),
    /// One connection.
    Connection(ConnectionKey),
    /// One external input.
    ExternalInput(AnyExternalInputKey),
    /// One external output.
    ExternalOutput(AnyExternalOutputKey),
}

/// A subject whose parent module instance can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum HierarchicalSubjectRef {
    /// A node. The authored node has no stored parent in this revision.
    Node(NodeKey),
    /// A module instance.
    ModuleInstance(ModuleInstanceKey),
}

/// Explicit correspondence from one removed subject to one new subject.
#[non_exhaustive]
pub enum SubjectReassociation<D> {
    /// Two nodes.
    Node {
        /// Base node.
        from: NodeKey,
        /// Target node.
        to: NodeKey,
        /// Migration rule for the pair.
        migration: NodeMigrationDirective<D>,
    },
    /// Two module instances.
    ModuleInstance {
        /// Base instance.
        from: ModuleInstanceKey,
        /// Target instance.
        to: ModuleInstanceKey,
        /// Migration rule for the pair.
        migration: ModuleMigrationDirective<D>,
    },
    /// Two input ports of the same signal kind.
    InPort {
        /// Base port.
        from: AnyInPortKey,
        /// Target port.
        to: AnyInPortKey,
    },
    /// Two output ports of the same signal kind.
    OutPort {
        /// Base port.
        from: AnyOutPortKey,
        /// Target port.
        to: AnyOutPortKey,
    },
    /// Two external inputs of the same signal kind.
    ExternalInput {
        /// Base input.
        from: AnyExternalInputKey,
        /// Target input.
        to: AnyExternalInputKey,
    },
    /// Two external outputs of the same signal kind.
    ExternalOutput {
        /// Base output.
        from: AnyExternalOutputKey,
        /// Target output.
        to: AnyExternalOutputKey,
    },
    #[doc(hidden)]
    __Domain(PhantomData<fn() -> D>, core::convert::Infallible),
}

impl<D> Clone for SubjectReassociation<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Node {
                from,
                to,
                migration,
            } => Self::Node {
                from: *from,
                to: *to,
                migration: *migration,
            },
            Self::ModuleInstance {
                from,
                to,
                migration,
            } => Self::ModuleInstance {
                from: *from,
                to: *to,
                migration: migration.clone(),
            },
            Self::InPort { from, to } => Self::InPort {
                from: *from,
                to: *to,
            },
            Self::OutPort { from, to } => Self::OutPort {
                from: *from,
                to: *to,
            },
            Self::ExternalInput { from, to } => Self::ExternalInput {
                from: *from,
                to: *to,
            },
            Self::ExternalOutput { from, to } => Self::ExternalOutput {
                from: *from,
                to: *to,
            },
            Self::__Domain(_, never) => match *never {},
        }
    }
}

impl<D> PartialEq for SubjectReassociation<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Node {
                    from: left_from,
                    to: left_to,
                    migration: left_migration,
                },
                Self::Node {
                    from: right_from,
                    to: right_to,
                    migration: right_migration,
                },
            ) => {
                left_from == right_from && left_to == right_to && left_migration == right_migration
            }
            (
                Self::ModuleInstance {
                    from: left_from,
                    to: left_to,
                    migration: left_migration,
                },
                Self::ModuleInstance {
                    from: right_from,
                    to: right_to,
                    migration: right_migration,
                },
            ) => {
                left_from == right_from && left_to == right_to && left_migration == right_migration
            }
            (
                Self::InPort {
                    from: left_from,
                    to: left_to,
                },
                Self::InPort {
                    from: right_from,
                    to: right_to,
                },
            ) => left_from == right_from && left_to == right_to,
            (
                Self::OutPort {
                    from: left_from,
                    to: left_to,
                },
                Self::OutPort {
                    from: right_from,
                    to: right_to,
                },
            ) => left_from == right_from && left_to == right_to,
            (
                Self::ExternalInput {
                    from: left_from,
                    to: left_to,
                },
                Self::ExternalInput {
                    from: right_from,
                    to: right_to,
                },
            ) => left_from == right_from && left_to == right_to,
            (
                Self::ExternalOutput {
                    from: left_from,
                    to: left_to,
                },
                Self::ExternalOutput {
                    from: right_from,
                    to: right_to,
                },
            ) => left_from == right_from && left_to == right_to,
            (Self::__Domain(_, never), Self::__Domain(_, other)) => match (*never, *other) {},
            _ => false,
        }
    }
}

impl<D> Eq for SubjectReassociation<D> {}

impl<D> fmt::Debug for SubjectReassociation<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node {
                from,
                to,
                migration,
            } => formatter
                .debug_struct("Node")
                .field("from", from)
                .field("to", to)
                .field("migration", migration)
                .finish(),
            Self::ModuleInstance {
                from,
                to,
                migration,
            } => formatter
                .debug_struct("ModuleInstance")
                .field("from", from)
                .field("to", to)
                .field("migration", migration)
                .finish(),
            Self::InPort { from, to } => formatter
                .debug_struct("InPort")
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::OutPort { from, to } => formatter
                .debug_struct("OutPort")
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::ExternalInput { from, to } => formatter
                .debug_struct("ExternalInput")
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::ExternalOutput { from, to } => formatter
                .debug_struct("ExternalOutput")
                .field("from", from)
                .field("to", to)
                .finish(),
            Self::__Domain(_, never) => match *never {},
        }
    }
}

/// One canonical declarative edit.
#[non_exhaustive]
pub enum PatchOperation<D> {
    /// Insert a node.
    AddNode(NodeDef<D>),
    /// Delete a node.
    RemoveNode {
        /// Node to delete.
        node: NodeKey,
    },
    /// Replace one node in place.
    ReplaceNode {
        /// Existing node key.
        node: NodeKey,
        /// Complete replacement definition. Its key equals `node`.
        replacement: NodeDef<D>,
        /// Migration rule for the surviving node.
        migration: NodeMigrationDirective<D>,
    },
    /// Insert a connection.
    AddConnection(ConnectionDef),
    /// Delete a connection.
    RemoveConnection {
        /// Connection to delete.
        connection: ConnectionKey,
    },
    /// Replace one connection in place.
    ReplaceConnection {
        /// Existing connection key.
        connection: ConnectionKey,
        /// Complete replacement. Its key equals `connection`.
        replacement: ConnectionDef,
    },
    /// Insert an external input.
    AddExternalInput(ExternalInputDef),
    /// Delete an external input.
    RemoveExternalInput {
        /// Input to delete.
        input: AnyExternalInputKey,
    },
    /// Replace one external input in place.
    ReplaceExternalInput {
        /// Existing input key.
        input: AnyExternalInputKey,
        /// Complete replacement. Its key equals `input`.
        replacement: ExternalInputDef,
    },
    /// Insert an external output.
    AddExternalOutput(ExternalOutputDef),
    /// Delete an external output.
    RemoveExternalOutput {
        /// Output to delete.
        output: AnyExternalOutputKey,
    },
    /// Replace one external output in place.
    ReplaceExternalOutput {
        /// Existing output key.
        output: AnyExternalOutputKey,
        /// Complete replacement. Its key equals `output`.
        replacement: ExternalOutputDef,
    },
    /// Insert a module instance.
    AddModuleInstance(ModuleInstanceDef<D>),
    /// Delete a module instance.
    RemoveModuleInstance {
        /// Instance to delete.
        module: ModuleInstanceKey,
    },
    /// Replace one module instance in place.
    ReplaceModuleInstance {
        /// Existing instance key.
        module: ModuleInstanceKey,
        /// Complete replacement. Its key equals `module`.
        replacement: ModuleInstanceDef<D>,
        /// Migration rule for the instance and its internals.
        migration: ModuleMigrationDirective<D>,
    },
    /// Assign a parent module instance.
    SetParent {
        /// Subject being reparented.
        subject: HierarchicalSubjectRef,
        /// Parent instance, or none for a root subject.
        parent: Option<ModuleInstanceKey>,
    },
    /// Replace diagnostic metadata.
    SetDiagnosticMeta {
        /// Subject whose metadata is replaced.
        subject: StructuralSubjectRef,
        /// Complete metadata value.
        meta: DiagnosticMeta,
    },
    /// Record an explicit correspondence.
    Reassociate(SubjectReassociation<D>),
}

impl<D> fmt::Debug for PatchOperation<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddNode(node) => formatter.debug_tuple("AddNode").field(node).finish(),
            Self::RemoveNode { node } => formatter
                .debug_struct("RemoveNode")
                .field("node", node)
                .finish(),
            Self::ReplaceNode {
                node,
                replacement,
                migration,
            } => formatter
                .debug_struct("ReplaceNode")
                .field("node", node)
                .field("replacement", replacement)
                .field("migration", migration)
                .finish(),
            Self::AddConnection(connection) => formatter
                .debug_tuple("AddConnection")
                .field(connection)
                .finish(),
            Self::RemoveConnection { connection } => formatter
                .debug_struct("RemoveConnection")
                .field("connection", connection)
                .finish(),
            Self::ReplaceConnection {
                connection,
                replacement,
            } => formatter
                .debug_struct("ReplaceConnection")
                .field("connection", connection)
                .field("replacement", replacement)
                .finish(),
            Self::AddExternalInput(input) => formatter
                .debug_tuple("AddExternalInput")
                .field(input)
                .finish(),
            Self::RemoveExternalInput { input } => formatter
                .debug_struct("RemoveExternalInput")
                .field("input", input)
                .finish(),
            Self::ReplaceExternalInput { input, replacement } => formatter
                .debug_struct("ReplaceExternalInput")
                .field("input", input)
                .field("replacement", replacement)
                .finish(),
            Self::AddExternalOutput(output) => formatter
                .debug_tuple("AddExternalOutput")
                .field(output)
                .finish(),
            Self::RemoveExternalOutput { output } => formatter
                .debug_struct("RemoveExternalOutput")
                .field("output", output)
                .finish(),
            Self::ReplaceExternalOutput {
                output,
                replacement,
            } => formatter
                .debug_struct("ReplaceExternalOutput")
                .field("output", output)
                .field("replacement", replacement)
                .finish(),
            Self::AddModuleInstance(instance) => formatter
                .debug_tuple("AddModuleInstance")
                .field(instance)
                .finish(),
            Self::RemoveModuleInstance { module } => formatter
                .debug_struct("RemoveModuleInstance")
                .field("module", module)
                .finish(),
            Self::ReplaceModuleInstance {
                module,
                replacement,
                migration,
            } => formatter
                .debug_struct("ReplaceModuleInstance")
                .field("module", module)
                .field("replacement", replacement)
                .field("migration", migration)
                .finish(),
            Self::SetParent { subject, parent } => formatter
                .debug_struct("SetParent")
                .field("subject", subject)
                .field("parent", parent)
                .finish(),
            Self::SetDiagnosticMeta { subject, meta } => formatter
                .debug_struct("SetDiagnosticMeta")
                .field("subject", subject)
                .field("meta", meta)
                .finish(),
            Self::Reassociate(mapping) => {
                formatter.debug_tuple("Reassociate").field(mapping).finish()
            }
        }
    }
}

impl<D: PartialEq> PartialEq for PatchOperation<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AddNode(left), Self::AddNode(right)) => left == right,
            (Self::RemoveNode { node: left }, Self::RemoveNode { node: right }) => left == right,
            (
                Self::ReplaceNode {
                    node: left_node,
                    replacement: left_replacement,
                    migration: left_migration,
                },
                Self::ReplaceNode {
                    node: right_node,
                    replacement: right_replacement,
                    migration: right_migration,
                },
            ) => {
                left_node == right_node
                    && left_replacement == right_replacement
                    && left_migration == right_migration
            }
            (Self::AddConnection(left), Self::AddConnection(right)) => left == right,
            (
                Self::RemoveConnection { connection: left },
                Self::RemoveConnection { connection: right },
            ) => left == right,
            (
                Self::ReplaceConnection {
                    connection: left_connection,
                    replacement: left_replacement,
                },
                Self::ReplaceConnection {
                    connection: right_connection,
                    replacement: right_replacement,
                },
            ) => left_connection == right_connection && left_replacement == right_replacement,
            (Self::AddExternalInput(left), Self::AddExternalInput(right)) => left == right,
            (
                Self::RemoveExternalInput { input: left },
                Self::RemoveExternalInput { input: right },
            ) => left == right,
            (
                Self::ReplaceExternalInput {
                    input: left_input,
                    replacement: left_replacement,
                },
                Self::ReplaceExternalInput {
                    input: right_input,
                    replacement: right_replacement,
                },
            ) => left_input == right_input && left_replacement == right_replacement,
            (Self::AddExternalOutput(left), Self::AddExternalOutput(right)) => left == right,
            (
                Self::RemoveExternalOutput { output: left },
                Self::RemoveExternalOutput { output: right },
            ) => left == right,
            (
                Self::ReplaceExternalOutput {
                    output: left_output,
                    replacement: left_replacement,
                },
                Self::ReplaceExternalOutput {
                    output: right_output,
                    replacement: right_replacement,
                },
            ) => left_output == right_output && left_replacement == right_replacement,
            (Self::AddModuleInstance(left), Self::AddModuleInstance(right)) => left == right,
            (
                Self::RemoveModuleInstance { module: left },
                Self::RemoveModuleInstance { module: right },
            ) => left == right,
            (
                Self::ReplaceModuleInstance {
                    module: left_module,
                    replacement: left_replacement,
                    migration: left_migration,
                },
                Self::ReplaceModuleInstance {
                    module: right_module,
                    replacement: right_replacement,
                    migration: right_migration,
                },
            ) => {
                left_module == right_module
                    && left_replacement == right_replacement
                    && left_migration == right_migration
            }
            (
                Self::SetParent {
                    subject: left_subject,
                    parent: left_parent,
                },
                Self::SetParent {
                    subject: right_subject,
                    parent: right_parent,
                },
            ) => left_subject == right_subject && left_parent == right_parent,
            (
                Self::SetDiagnosticMeta {
                    subject: left_subject,
                    meta: left_meta,
                },
                Self::SetDiagnosticMeta {
                    subject: right_subject,
                    meta: right_meta,
                },
            ) => left_subject == right_subject && left_meta == right_meta,
            (Self::Reassociate(left), Self::Reassociate(right)) => left == right,
            _ => false,
        }
    }
}

impl<D: PartialEq> Eq for PatchOperation<D> {}

/// Borrowed iterator of normalized patch operations.
pub struct PatchOperationIter<'a, D> {
    inner: core::slice::Iter<'a, PatchOperation<D>>,
}

impl<'a, D> Iterator for PatchOperationIter<'a, D> {
    type Item = &'a PatchOperation<D>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<D> ExactSizeIterator for PatchOperationIter<'_, D> {}

/// Owned declarative rewrite bound to one base topology.
#[derive(Debug)]
pub struct NetworkPatch<D> {
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    time_domain_id: TimeDomainId,
    base_revision: NetworkRevision,
    operations: Vec<PatchOperation<D>>,
}

impl<D> NetworkPatch<D> {
    /// Returns the network this patch rewrites.
    #[must_use]
    pub const fn network_key(&self) -> NetworkKey {
        self.network_key
    }

    /// Returns the base topology revision named by the patch.
    #[must_use]
    pub const fn base_revision(&self) -> NetworkRevision {
        self.base_revision
    }

    /// Returns the base network fingerprint the patch was bound to.
    #[must_use]
    pub const fn base_fingerprint(&self) -> NetworkFingerprint {
        self.fingerprint
    }

    /// Returns the logical time domain copied from the base topology.
    #[must_use]
    pub const fn time_domain_id(&self) -> TimeDomainId {
        self.time_domain_id
    }

    /// Returns the normalized operations in canonical family order.
    #[must_use]
    pub fn operations(&self) -> PatchOperationIter<'_, D> {
        PatchOperationIter {
            inner: self.operations.iter(),
        }
    }
}

/// Local construction failure. These are not preparation reports.
#[derive(Eq)]
#[non_exhaustive]
pub enum PatchBuildFailure<D> {
    /// An operation names a network other than the bound base.
    ForeignArtifact {
        /// Subject that carried the foreign identity.
        subject: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    /// The same subject was added twice with different definitions.
    DuplicateOperation {
        /// Subject named by both additions.
        subject: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    /// Two edits of one subject cannot be applied together.
    ConflictingEdit {
        /// Subject of the conflicting edits.
        subject: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    /// A replacement definition uses a different key from its subject.
    InvalidReplacementKey {
        /// Subject the operation named.
        subject: SubjectRef,
        /// Key carried by the replacement definition.
        replacement: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    /// A reassociation is self-mapped, non-injective, or kind-incompatible.
    InvalidReassociation {
        /// Proposed source.
        source: SubjectRef,
        /// Proposed target.
        target: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
    /// Two parent assignments disagree.
    ContradictoryHierarchy {
        /// Subject of the parent assignments.
        subject: SubjectRef,
        marker: PhantomData<fn() -> D>,
    },
}

impl<D> PatchBuildFailure<D> {
    /// Projects this failure into the catalogue problem model.
    #[must_use]
    pub fn problem(&self) -> Problem<D> {
        match self {
            Self::ForeignArtifact { subject, .. } => Problem::new(
                subject.clone(),
                Vec::new(),
                ProblemEvidence::ReconfigurationForeignArtifact {
                    marker: PhantomData,
                },
            ),
            Self::DuplicateOperation { subject, .. } => Problem::new(
                subject.clone(),
                Vec::new(),
                ProblemEvidence::ReconfigurationDuplicateOperation {
                    marker: PhantomData,
                },
            ),
            Self::ConflictingEdit { subject, .. } => Problem::new(
                subject.clone(),
                Vec::new(),
                ProblemEvidence::ReconfigurationConflictingEdit {
                    marker: PhantomData,
                },
            ),
            Self::InvalidReplacementKey {
                subject,
                replacement,
                ..
            } => Problem::new(
                subject.clone(),
                vec![RelatedSubject {
                    role: RelatedSubjectRole::TargetSubject,
                    subject: replacement.clone(),
                }],
                ProblemEvidence::ReconfigurationInvalidReplacementKey {
                    marker: PhantomData,
                },
            ),
            Self::InvalidReassociation { source, target, .. } => Problem::new(
                source.clone(),
                vec![RelatedSubject {
                    role: RelatedSubjectRole::MigrationTarget,
                    subject: target.clone(),
                }],
                ProblemEvidence::ReconfigurationInvalidReassociation {
                    source: source.clone(),
                    target: target.clone(),
                    marker: PhantomData,
                },
            ),
            Self::ContradictoryHierarchy { subject, .. } => Problem::new(
                subject.clone(),
                Vec::new(),
                ProblemEvidence::ReconfigurationContradictoryHierarchy {
                    marker: PhantomData,
                },
            ),
        }
    }
}

impl<D> Clone for PatchBuildFailure<D> {
    fn clone(&self) -> Self {
        match self {
            Self::ForeignArtifact { subject, marker } => Self::ForeignArtifact {
                subject: subject.clone(),
                marker: *marker,
            },
            Self::DuplicateOperation { subject, marker } => Self::DuplicateOperation {
                subject: subject.clone(),
                marker: *marker,
            },
            Self::ConflictingEdit { subject, marker } => Self::ConflictingEdit {
                subject: subject.clone(),
                marker: *marker,
            },
            Self::InvalidReplacementKey {
                subject,
                replacement,
                marker,
            } => Self::InvalidReplacementKey {
                subject: subject.clone(),
                replacement: replacement.clone(),
                marker: *marker,
            },
            Self::InvalidReassociation {
                source,
                target,
                marker,
            } => Self::InvalidReassociation {
                source: source.clone(),
                target: target.clone(),
                marker: *marker,
            },
            Self::ContradictoryHierarchy { subject, marker } => Self::ContradictoryHierarchy {
                subject: subject.clone(),
                marker: *marker,
            },
        }
    }
}

impl<D> PartialEq for PatchBuildFailure<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::ForeignArtifact { subject: left, .. },
                Self::ForeignArtifact { subject: right, .. },
            )
            | (
                Self::DuplicateOperation { subject: left, .. },
                Self::DuplicateOperation { subject: right, .. },
            )
            | (
                Self::ConflictingEdit { subject: left, .. },
                Self::ConflictingEdit { subject: right, .. },
            )
            | (
                Self::ContradictoryHierarchy { subject: left, .. },
                Self::ContradictoryHierarchy { subject: right, .. },
            ) => left == right,
            (
                Self::InvalidReplacementKey {
                    subject: left_subject,
                    replacement: left_replacement,
                    ..
                },
                Self::InvalidReplacementKey {
                    subject: right_subject,
                    replacement: right_replacement,
                    ..
                },
            ) => left_subject == right_subject && left_replacement == right_replacement,
            (
                Self::InvalidReassociation {
                    source: left_source,
                    target: left_target,
                    ..
                },
                Self::InvalidReassociation {
                    source: right_source,
                    target: right_target,
                    ..
                },
            ) => left_source == right_source && left_target == right_target,
            _ => false,
        }
    }
}

impl<D> fmt::Debug for PatchBuildFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignArtifact { subject, .. } => formatter
                .debug_struct("ForeignArtifact")
                .field("subject", subject)
                .finish(),
            Self::DuplicateOperation { subject, .. } => formatter
                .debug_struct("DuplicateOperation")
                .field("subject", subject)
                .finish(),
            Self::ConflictingEdit { subject, .. } => formatter
                .debug_struct("ConflictingEdit")
                .field("subject", subject)
                .finish(),
            Self::InvalidReplacementKey {
                subject,
                replacement,
                ..
            } => formatter
                .debug_struct("InvalidReplacementKey")
                .field("subject", subject)
                .field("replacement", replacement)
                .finish(),
            Self::InvalidReassociation { source, target, .. } => formatter
                .debug_struct("InvalidReassociation")
                .field("source", source)
                .field("target", target)
                .finish(),
            Self::ContradictoryHierarchy { subject, .. } => formatter
                .debug_struct("ContradictoryHierarchy")
                .field("subject", subject)
                .finish(),
        }
    }
}

impl<D> fmt::Display for PatchBuildFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.problem().code().as_str())
    }
}

impl<D> std::error::Error for PatchBuildFailure<D> {}

#[derive(Clone)]
enum NodeSlot<D> {
    Add(NodeDef<D>),
    Remove,
    Replace(NodeDef<D>, NodeMigrationDirective<D>),
}

#[derive(Clone)]
enum PlainSlot<T> {
    Add(T),
    Remove,
    Replace(T),
}

#[derive(Clone)]
enum ModuleSlot<D> {
    Add(ModuleInstanceDef<D>),
    Remove,
    Replace(ModuleInstanceDef<D>, ModuleMigrationDirective<D>),
}

/// Builder for one normalized [`NetworkPatch`].
///
/// The builder owns its operations. Identical edits are ignored. Contradictions
/// visible from one operation fail immediately. `finish` is infallible.
pub struct NetworkPatchBuilder<D> {
    network_key: NetworkKey,
    fingerprint: NetworkFingerprint,
    time_domain_id: TimeDomainId,
    base_revision: NetworkRevision,
    nodes: BTreeMap<NodeKey, NodeSlot<D>>,
    connections: BTreeMap<ConnectionKey, PlainSlot<ConnectionDef>>,
    external_inputs: BTreeMap<AnyExternalInputKey, PlainSlot<ExternalInputDef>>,
    external_outputs: BTreeMap<AnyExternalOutputKey, PlainSlot<ExternalOutputDef>>,
    modules: BTreeMap<ModuleInstanceKey, ModuleSlot<D>>,
    parents: BTreeMap<HierarchicalSubjectRef, Option<ModuleInstanceKey>>,
    metas: BTreeMap<StructuralSubjectRef, DiagnosticMeta>,
    reassociations: Vec<SubjectReassociation<D>>,
}

impl<D> NetworkPatchBuilder<D> {
    /// Returns the base revision this builder was bound to.
    #[must_use]
    pub const fn base_revision(&self) -> NetworkRevision {
        self.base_revision
    }

    /// Returns the base fingerprint this builder was bound to.
    #[must_use]
    pub const fn base_fingerprint(&self) -> NetworkFingerprint {
        self.fingerprint
    }

    pub(crate) fn bound(
        network_key: NetworkKey,
        fingerprint: NetworkFingerprint,
        time_domain_id: TimeDomainId,
        base_revision: NetworkRevision,
    ) -> Self {
        Self {
            network_key,
            fingerprint,
            time_domain_id,
            base_revision,
            nodes: BTreeMap::new(),
            connections: BTreeMap::new(),
            external_inputs: BTreeMap::new(),
            external_outputs: BTreeMap::new(),
            modules: BTreeMap::new(),
            parents: BTreeMap::new(),
            metas: BTreeMap::new(),
            reassociations: Vec::new(),
        }
    }

    /// Adds one node.
    pub fn add_node(mut self, node: NodeDef<D>) -> Result<Self, PatchBuildFailure<D>> {
        let key = node.key();
        match self.nodes.get(&key) {
            None => {
                self.nodes.insert(key, NodeSlot::Add(node));
                Ok(self)
            }
            Some(NodeSlot::Add(existing)) if existing == &node => Ok(self),
            Some(NodeSlot::Add(_)) => Err(duplicate(SubjectRef::Node(key))),
            Some(_) => Err(conflict(SubjectRef::Node(key))),
        }
    }

    /// Removes one node.
    pub fn remove_node(mut self, node: NodeKey) -> Result<Self, PatchBuildFailure<D>> {
        if self
            .parents
            .contains_key(&HierarchicalSubjectRef::Node(node))
            || self.metas.contains_key(&StructuralSubjectRef::Node(node))
        {
            return Err(conflict(SubjectRef::Node(node)));
        }
        match self.nodes.get(&node) {
            None => {
                self.nodes.insert(node, NodeSlot::Remove);
                Ok(self)
            }
            Some(NodeSlot::Remove) => Ok(self),
            Some(_) => Err(conflict(SubjectRef::Node(node))),
        }
    }

    /// Replaces one node. The replacement key must equal `node`.
    pub fn replace_node(
        mut self,
        node: NodeKey,
        replacement: NodeDef<D>,
        migration: NodeMigrationDirective<D>,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if replacement.key() != node {
            return Err(invalid_key(
                SubjectRef::Node(node),
                SubjectRef::Node(replacement.key()),
            ));
        }
        if self
            .parents
            .contains_key(&HierarchicalSubjectRef::Node(node))
            || self.metas.contains_key(&StructuralSubjectRef::Node(node))
        {
            return Err(conflict(SubjectRef::Node(node)));
        }
        match self.nodes.get(&node) {
            None => {
                self.nodes
                    .insert(node, NodeSlot::Replace(replacement, migration));
                Ok(self)
            }
            Some(NodeSlot::Replace(existing, directive))
                if existing == &replacement && *directive == migration =>
            {
                Ok(self)
            }
            Some(_) => Err(conflict(SubjectRef::Node(node))),
        }
    }

    /// Adds one connection.
    pub fn add_connection(
        mut self,
        connection: ConnectionDef,
    ) -> Result<Self, PatchBuildFailure<D>> {
        insert_plain(
            &mut self.connections,
            connection.key(),
            PlainSlot::Add(connection),
            SubjectRef::Connection,
        )
        .map(|()| self)
    }

    /// Removes one connection.
    pub fn remove_connection(
        mut self,
        connection: ConnectionKey,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if self
            .metas
            .contains_key(&StructuralSubjectRef::Connection(connection))
        {
            return Err(conflict(SubjectRef::Connection(connection)));
        }
        insert_remove(
            &mut self.connections,
            connection,
            SubjectRef::Connection(connection),
        )
        .map(|()| self)
    }

    /// Replaces one connection. The replacement key must equal `connection`.
    pub fn replace_connection(
        mut self,
        connection: ConnectionKey,
        replacement: ConnectionDef,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if replacement.key() != connection {
            return Err(invalid_key(
                SubjectRef::Connection(connection),
                SubjectRef::Connection(replacement.key()),
            ));
        }
        if self
            .metas
            .contains_key(&StructuralSubjectRef::Connection(connection))
        {
            return Err(conflict(SubjectRef::Connection(connection)));
        }
        insert_replace(
            &mut self.connections,
            connection,
            replacement,
            SubjectRef::Connection(connection),
        )
        .map(|()| self)
    }

    /// Adds one external input.
    pub fn add_external_input(
        mut self,
        input: ExternalInputDef,
    ) -> Result<Self, PatchBuildFailure<D>> {
        insert_plain(
            &mut self.external_inputs,
            input.key(),
            PlainSlot::Add(input),
            SubjectRef::ExternalInput,
        )
        .map(|()| self)
    }

    /// Removes one external input.
    pub fn remove_external_input(
        mut self,
        input: AnyExternalInputKey,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if self
            .metas
            .contains_key(&StructuralSubjectRef::ExternalInput(input))
        {
            return Err(conflict(SubjectRef::ExternalInput(input)));
        }
        insert_remove(
            &mut self.external_inputs,
            input,
            SubjectRef::ExternalInput(input),
        )
        .map(|()| self)
    }

    /// Replaces one external input. The replacement key must equal `input`.
    pub fn replace_external_input(
        mut self,
        input: AnyExternalInputKey,
        replacement: ExternalInputDef,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if replacement.key() != input {
            return Err(invalid_key(
                SubjectRef::ExternalInput(input),
                SubjectRef::ExternalInput(replacement.key()),
            ));
        }
        if self
            .metas
            .contains_key(&StructuralSubjectRef::ExternalInput(input))
        {
            return Err(conflict(SubjectRef::ExternalInput(input)));
        }
        insert_replace(
            &mut self.external_inputs,
            input,
            replacement,
            SubjectRef::ExternalInput(input),
        )
        .map(|()| self)
    }

    /// Adds one external output.
    pub fn add_external_output(
        mut self,
        output: ExternalOutputDef,
    ) -> Result<Self, PatchBuildFailure<D>> {
        insert_plain(
            &mut self.external_outputs,
            output.key(),
            PlainSlot::Add(output),
            SubjectRef::ExternalOutput,
        )
        .map(|()| self)
    }

    /// Removes one external output.
    pub fn remove_external_output(
        mut self,
        output: AnyExternalOutputKey,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if self
            .metas
            .contains_key(&StructuralSubjectRef::ExternalOutput(output))
        {
            return Err(conflict(SubjectRef::ExternalOutput(output)));
        }
        insert_remove(
            &mut self.external_outputs,
            output,
            SubjectRef::ExternalOutput(output),
        )
        .map(|()| self)
    }

    /// Replaces one external output. The replacement key must equal `output`.
    pub fn replace_external_output(
        mut self,
        output: AnyExternalOutputKey,
        replacement: ExternalOutputDef,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if replacement.key() != output {
            return Err(invalid_key(
                SubjectRef::ExternalOutput(output),
                SubjectRef::ExternalOutput(replacement.key()),
            ));
        }
        if self
            .metas
            .contains_key(&StructuralSubjectRef::ExternalOutput(output))
        {
            return Err(conflict(SubjectRef::ExternalOutput(output)));
        }
        insert_replace(
            &mut self.external_outputs,
            output,
            replacement,
            SubjectRef::ExternalOutput(output),
        )
        .map(|()| self)
    }

    /// Adds one module instance.
    pub fn add_module_instance(
        mut self,
        instance: ModuleInstanceDef<D>,
    ) -> Result<Self, PatchBuildFailure<D>> {
        let key = instance.key();
        if let Some(parent) = self
            .parents
            .get(&HierarchicalSubjectRef::ModuleInstance(key))
        {
            if *parent != instance.parent() {
                return Err(hierarchy(SubjectRef::ModuleInstance(key)));
            }
        }
        match self.modules.get(&key) {
            None => {
                self.modules.insert(key, ModuleSlot::Add(instance));
                Ok(self)
            }
            Some(ModuleSlot::Add(existing)) if existing == &instance => Ok(self),
            Some(ModuleSlot::Add(_)) => Err(duplicate(SubjectRef::ModuleInstance(key))),
            Some(_) => Err(conflict(SubjectRef::ModuleInstance(key))),
        }
    }

    /// Removes one module instance.
    pub fn remove_module_instance(
        mut self,
        module: ModuleInstanceKey,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if self
            .parents
            .contains_key(&HierarchicalSubjectRef::ModuleInstance(module))
            || self
                .metas
                .contains_key(&StructuralSubjectRef::Module(module))
        {
            return Err(conflict(SubjectRef::ModuleInstance(module)));
        }
        match self.modules.get(&module) {
            None => {
                self.modules.insert(module, ModuleSlot::Remove);
                Ok(self)
            }
            Some(ModuleSlot::Remove) => Ok(self),
            Some(_) => Err(conflict(SubjectRef::ModuleInstance(module))),
        }
    }

    /// Replaces one module instance. The replacement key must equal `module`.
    pub fn replace_module_instance(
        mut self,
        module: ModuleInstanceKey,
        replacement: ModuleInstanceDef<D>,
        migration: ModuleMigrationDirective<D>,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if replacement.key() != module {
            return Err(invalid_key(
                SubjectRef::ModuleInstance(module),
                SubjectRef::ModuleInstance(replacement.key()),
            ));
        }
        if let Some(parent) = self
            .parents
            .get(&HierarchicalSubjectRef::ModuleInstance(module))
        {
            if *parent != replacement.parent() {
                return Err(hierarchy(SubjectRef::ModuleInstance(module)));
            }
        }
        if self
            .metas
            .contains_key(&StructuralSubjectRef::Module(module))
        {
            return Err(conflict(SubjectRef::ModuleInstance(module)));
        }
        match self.modules.get(&module) {
            None => {
                self.modules
                    .insert(module, ModuleSlot::Replace(replacement, migration));
                Ok(self)
            }
            Some(ModuleSlot::Replace(existing, directive))
                if existing == &replacement && directive == &migration =>
            {
                Ok(self)
            }
            Some(_) => Err(conflict(SubjectRef::ModuleInstance(module))),
        }
    }

    /// Assigns the parent of a node or module instance.
    pub fn set_parent(
        mut self,
        subject: HierarchicalSubjectRef,
        parent: Option<ModuleInstanceKey>,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if let Some(existing) = self.parents.get(&subject) {
            return if *existing == parent {
                Ok(self)
            } else {
                Err(hierarchy(hierarchy_subject(subject)))
            };
        }
        match subject {
            HierarchicalSubjectRef::Node(node) => {
                if matches!(self.nodes.get(&node), Some(NodeSlot::Remove)) {
                    return Err(conflict(SubjectRef::Node(node)));
                }
            }
            HierarchicalSubjectRef::ModuleInstance(module) => {
                if matches!(self.modules.get(&module), Some(ModuleSlot::Remove)) {
                    return Err(conflict(SubjectRef::ModuleInstance(module)));
                }
                let current = match self.modules.get(&module) {
                    Some(ModuleSlot::Add(instance) | ModuleSlot::Replace(instance, _)) => {
                        Some(instance.parent())
                    }
                    _ => None,
                };
                if let Some(current) = current {
                    if current != parent {
                        return Err(hierarchy(SubjectRef::ModuleInstance(module)));
                    }
                }
            }
        }
        self.parents.insert(subject, parent);
        Ok(self)
    }

    /// Replaces diagnostic metadata for one structural subject.
    pub fn set_diagnostic_meta(
        mut self,
        subject: StructuralSubjectRef,
        meta: DiagnosticMeta,
    ) -> Result<Self, PatchBuildFailure<D>> {
        if let StructuralSubjectRef::Network(key) = subject {
            if key != self.network_key {
                return Err(PatchBuildFailure::ForeignArtifact {
                    subject: SubjectRef::Network(key),
                    marker: PhantomData,
                });
            }
        }
        if subject_removed(&self, subject) {
            return Err(conflict(structural_subject(subject)));
        }
        match self.metas.get(&subject) {
            Some(existing) if existing == &meta => Ok(self),
            Some(_) => Err(conflict(structural_subject(subject))),
            None => {
                self.metas.insert(subject, meta);
                Ok(self)
            }
        }
    }

    /// Records one explicit reassociation.
    pub fn reassociate(
        mut self,
        mapping: SubjectReassociation<D>,
    ) -> Result<Self, PatchBuildFailure<D>> {
        let (source, target) = reassoc_ends(&mapping);
        if source == target
            || !reassoc_kinds_match(&mapping)
            || reassoc_conflicts(&self.reassociations, &source, &target)
        {
            return Err(PatchBuildFailure::InvalidReassociation {
                source,
                target,
                marker: PhantomData,
            });
        }
        if self
            .reassociations
            .iter()
            .any(|existing| existing == &mapping)
        {
            return Ok(self);
        }
        self.reassociations.push(mapping);
        Ok(self)
    }

    /// Finishes the builder into one normalized patch.
    #[must_use]
    pub fn finish(self) -> NetworkPatch<D> {
        let Self {
            network_key,
            fingerprint,
            time_domain_id,
            base_revision,
            nodes,
            connections,
            external_inputs,
            external_outputs,
            modules,
            mut parents,
            mut metas,
            mut reassociations,
        } = self;
        let mut operations = Vec::new();
        for (key, slot) in nodes {
            match slot {
                NodeSlot::Add(mut node) => {
                    if let Some(meta) = metas.remove(&StructuralSubjectRef::Node(key)) {
                        node = node_with_meta(node, meta);
                    }
                    operations.push(PatchOperation::AddNode(node));
                }
                NodeSlot::Remove => operations.push(PatchOperation::RemoveNode { node: key }),
                NodeSlot::Replace(mut replacement, migration) => {
                    if let Some(meta) = metas.remove(&StructuralSubjectRef::Node(key)) {
                        replacement = node_with_meta(replacement, meta);
                    }
                    operations.push(PatchOperation::ReplaceNode {
                        node: key,
                        replacement,
                        migration,
                    });
                }
            }
        }
        push_plain(
            connections,
            &mut metas,
            StructuralSubjectRef::Connection,
            &mut operations,
            PatchOperation::AddConnection,
            |connection| PatchOperation::RemoveConnection { connection },
            |connection, replacement| PatchOperation::ReplaceConnection {
                connection,
                replacement,
            },
            connection_with_meta,
        );
        push_plain(
            external_inputs,
            &mut metas,
            StructuralSubjectRef::ExternalInput,
            &mut operations,
            PatchOperation::AddExternalInput,
            |input| PatchOperation::RemoveExternalInput { input },
            |input, replacement| PatchOperation::ReplaceExternalInput { input, replacement },
            input_with_meta,
        );
        push_plain(
            external_outputs,
            &mut metas,
            StructuralSubjectRef::ExternalOutput,
            &mut operations,
            PatchOperation::AddExternalOutput,
            |output| PatchOperation::RemoveExternalOutput { output },
            |output, replacement| PatchOperation::ReplaceExternalOutput {
                output,
                replacement,
            },
            output_with_meta,
        );
        for (key, slot) in modules {
            match slot {
                ModuleSlot::Add(mut instance) => {
                    if let Some(parent) =
                        parents.remove(&HierarchicalSubjectRef::ModuleInstance(key))
                    {
                        instance = module_with_parent(instance, parent);
                    }
                    if let Some(meta) = metas.remove(&StructuralSubjectRef::Module(key)) {
                        instance = module_with_meta(instance, meta);
                    }
                    operations.push(PatchOperation::AddModuleInstance(instance));
                }
                ModuleSlot::Remove => {
                    operations.push(PatchOperation::RemoveModuleInstance { module: key })
                }
                ModuleSlot::Replace(mut replacement, migration) => {
                    if let Some(parent) =
                        parents.remove(&HierarchicalSubjectRef::ModuleInstance(key))
                    {
                        replacement = module_with_parent(replacement, parent);
                    }
                    if let Some(meta) = metas.remove(&StructuralSubjectRef::Module(key)) {
                        replacement = module_with_meta(replacement, meta);
                    }
                    operations.push(PatchOperation::ReplaceModuleInstance {
                        module: key,
                        replacement,
                        migration,
                    });
                }
            }
        }
        for (subject, parent) in parents {
            operations.push(PatchOperation::SetParent { subject, parent });
        }
        for (subject, meta) in metas {
            operations.push(PatchOperation::SetDiagnosticMeta { subject, meta });
        }
        reassociations.sort_by_key(reassoc_rank);
        operations.extend(reassociations.into_iter().map(PatchOperation::Reassociate));
        NetworkPatch {
            network_key,
            fingerprint,
            time_domain_id,
            base_revision,
            operations,
        }
    }
}

fn duplicate<D>(subject: SubjectRef) -> PatchBuildFailure<D> {
    PatchBuildFailure::DuplicateOperation {
        subject,
        marker: PhantomData,
    }
}

fn conflict<D>(subject: SubjectRef) -> PatchBuildFailure<D> {
    PatchBuildFailure::ConflictingEdit {
        subject,
        marker: PhantomData,
    }
}

fn invalid_key<D>(subject: SubjectRef, replacement: SubjectRef) -> PatchBuildFailure<D> {
    PatchBuildFailure::InvalidReplacementKey {
        subject,
        replacement,
        marker: PhantomData,
    }
}

fn hierarchy<D>(subject: SubjectRef) -> PatchBuildFailure<D> {
    PatchBuildFailure::ContradictoryHierarchy {
        subject,
        marker: PhantomData,
    }
}

fn hierarchy_subject(subject: HierarchicalSubjectRef) -> SubjectRef {
    match subject {
        HierarchicalSubjectRef::Node(node) => SubjectRef::Node(node),
        HierarchicalSubjectRef::ModuleInstance(module) => SubjectRef::ModuleInstance(module),
    }
}

fn structural_subject(subject: StructuralSubjectRef) -> SubjectRef {
    match subject {
        StructuralSubjectRef::Network(key) => SubjectRef::Network(key),
        StructuralSubjectRef::Module(key) => SubjectRef::ModuleInstance(key),
        StructuralSubjectRef::Node(key) => SubjectRef::Node(key),
        StructuralSubjectRef::InPort(key) => SubjectRef::InPort(key),
        StructuralSubjectRef::OutPort(key) => SubjectRef::OutPort(key),
        StructuralSubjectRef::Connection(key) => SubjectRef::Connection(key),
        StructuralSubjectRef::ExternalInput(key) => SubjectRef::ExternalInput(key),
        StructuralSubjectRef::ExternalOutput(key) => SubjectRef::ExternalOutput(key),
    }
}

fn subject_removed<D>(builder: &NetworkPatchBuilder<D>, subject: StructuralSubjectRef) -> bool {
    match subject {
        StructuralSubjectRef::Node(key) => {
            matches!(builder.nodes.get(&key), Some(NodeSlot::Remove))
        }
        StructuralSubjectRef::Connection(key) => {
            matches!(builder.connections.get(&key), Some(PlainSlot::Remove))
        }
        StructuralSubjectRef::ExternalInput(key) => {
            matches!(builder.external_inputs.get(&key), Some(PlainSlot::Remove))
        }
        StructuralSubjectRef::ExternalOutput(key) => {
            matches!(builder.external_outputs.get(&key), Some(PlainSlot::Remove))
        }
        StructuralSubjectRef::Module(key) => {
            matches!(builder.modules.get(&key), Some(ModuleSlot::Remove))
        }
        StructuralSubjectRef::Network(_)
        | StructuralSubjectRef::InPort(_)
        | StructuralSubjectRef::OutPort(_) => false,
    }
}

fn insert_plain<K, T, D>(
    map: &mut BTreeMap<K, PlainSlot<T>>,
    key: K,
    slot: PlainSlot<T>,
    subject: impl Fn(K) -> SubjectRef,
) -> Result<(), PatchBuildFailure<D>>
where
    K: Ord + Copy,
    T: PartialEq,
{
    match map.get(&key) {
        None => {
            map.insert(key, slot);
            Ok(())
        }
        Some(existing) if plain_same(existing, &slot) => Ok(()),
        Some(PlainSlot::Add(_)) => Err(duplicate(subject(key))),
        Some(_) => Err(conflict(subject(key))),
    }
}

fn insert_remove<K, T, D>(
    map: &mut BTreeMap<K, PlainSlot<T>>,
    key: K,
    subject: SubjectRef,
) -> Result<(), PatchBuildFailure<D>>
where
    K: Ord + Copy,
{
    match map.get(&key) {
        None => {
            map.insert(key, PlainSlot::Remove);
            Ok(())
        }
        Some(PlainSlot::Remove) => Ok(()),
        Some(_) => Err(conflict(subject)),
    }
}

fn insert_replace<K, T, D>(
    map: &mut BTreeMap<K, PlainSlot<T>>,
    key: K,
    replacement: T,
    subject: SubjectRef,
) -> Result<(), PatchBuildFailure<D>>
where
    K: Ord + Copy,
    T: PartialEq,
{
    match map.get(&key) {
        None => {
            map.insert(key, PlainSlot::Replace(replacement));
            Ok(())
        }
        Some(PlainSlot::Replace(existing)) if existing == &replacement => Ok(()),
        Some(_) => Err(conflict(subject)),
    }
}

fn plain_same<T: PartialEq>(left: &PlainSlot<T>, right: &PlainSlot<T>) -> bool {
    match (left, right) {
        (PlainSlot::Add(left), PlainSlot::Add(right))
        | (PlainSlot::Replace(left), PlainSlot::Replace(right)) => left == right,
        (PlainSlot::Remove, PlainSlot::Remove) => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_plain<K, T, D>(
    slots: BTreeMap<K, PlainSlot<T>>,
    metas: &mut BTreeMap<StructuralSubjectRef, DiagnosticMeta>,
    subject_of: impl Fn(K) -> StructuralSubjectRef,
    operations: &mut Vec<PatchOperation<D>>,
    add: impl Fn(T) -> PatchOperation<D>,
    remove: impl Fn(K) -> PatchOperation<D>,
    replace: impl Fn(K, T) -> PatchOperation<D>,
    with_meta: impl Fn(T, DiagnosticMeta) -> T,
) where
    K: Copy,
{
    for (key, slot) in slots {
        match slot {
            PlainSlot::Add(mut value) => {
                if let Some(meta) = metas.remove(&subject_of(key)) {
                    value = with_meta(value, meta);
                }
                operations.push(add(value));
            }
            PlainSlot::Remove => operations.push(remove(key)),
            PlainSlot::Replace(mut value) => {
                if let Some(meta) = metas.remove(&subject_of(key)) {
                    value = with_meta(value, meta);
                }
                operations.push(replace(key, value));
            }
        }
    }
}

fn node_with_meta<D>(node: NodeDef<D>, meta: DiagnosticMeta) -> NodeDef<D> {
    let (key, kind, ports, _) = node.into_parts();
    NodeDef::new(key, kind, ports, meta)
}

fn connection_with_meta(connection: ConnectionDef, meta: DiagnosticMeta) -> ConnectionDef {
    let (key, from, to, _) = connection.into_parts();
    ConnectionDef::new(key, from, to, meta)
}

fn input_with_meta(input: ExternalInputDef, meta: DiagnosticMeta) -> ExternalInputDef {
    ExternalInputDef::new(input.key(), meta)
}

fn output_with_meta(output: ExternalOutputDef, meta: DiagnosticMeta) -> ExternalOutputDef {
    ExternalOutputDef::new(output.key(), output.source(), meta)
}

fn module_with_meta<D>(
    instance: ModuleInstanceDef<D>,
    meta: DiagnosticMeta,
) -> ModuleInstanceDef<D> {
    ModuleInstanceDef::new(
        instance.key(),
        instance.module().clone(),
        instance.bindings().clone(),
        instance.parent(),
        meta,
    )
}

fn module_with_parent<D>(
    instance: ModuleInstanceDef<D>,
    parent: Option<ModuleInstanceKey>,
) -> ModuleInstanceDef<D> {
    ModuleInstanceDef::new(
        instance.key(),
        instance.module().clone(),
        instance.bindings().clone(),
        parent,
        instance.meta().clone(),
    )
}

fn reassoc_ends<D>(mapping: &SubjectReassociation<D>) -> (SubjectRef, SubjectRef) {
    match mapping {
        SubjectReassociation::Node { from, to, .. } => {
            (SubjectRef::Node(*from), SubjectRef::Node(*to))
        }
        SubjectReassociation::ModuleInstance { from, to, .. } => (
            SubjectRef::ModuleInstance(*from),
            SubjectRef::ModuleInstance(*to),
        ),
        SubjectReassociation::InPort { from, to } => {
            (SubjectRef::InPort(*from), SubjectRef::InPort(*to))
        }
        SubjectReassociation::OutPort { from, to } => {
            (SubjectRef::OutPort(*from), SubjectRef::OutPort(*to))
        }
        SubjectReassociation::ExternalInput { from, to } => (
            SubjectRef::ExternalInput(*from),
            SubjectRef::ExternalInput(*to),
        ),
        SubjectReassociation::ExternalOutput { from, to } => (
            SubjectRef::ExternalOutput(*from),
            SubjectRef::ExternalOutput(*to),
        ),
        SubjectReassociation::__Domain(_, never) => match *never {},
    }
}

fn reassoc_kinds_match<D>(mapping: &SubjectReassociation<D>) -> bool {
    match mapping {
        SubjectReassociation::Node { .. }
        | SubjectReassociation::ModuleInstance { .. }
        | SubjectReassociation::__Domain(_, _) => true,
        SubjectReassociation::InPort { from, to } => from.kind() == to.kind(),
        SubjectReassociation::OutPort { from, to } => from.kind() == to.kind(),
        SubjectReassociation::ExternalInput { from, to } => from.kind() == to.kind(),
        SubjectReassociation::ExternalOutput { from, to } => from.kind() == to.kind(),
    }
}

fn reassoc_conflicts<D>(
    existing: &[SubjectReassociation<D>],
    source: &SubjectRef,
    target: &SubjectRef,
) -> bool {
    existing.iter().any(|mapping| {
        let (from, to) = reassoc_ends(mapping);
        &from == source || &to == target || &from == target || &to == source
    })
}

fn reassoc_rank<D>(mapping: &SubjectReassociation<D>) -> (u8, SubjectRef, SubjectRef) {
    let (source, target) = reassoc_ends(mapping);
    let family = match mapping {
        SubjectReassociation::Node { .. } => 0,
        SubjectReassociation::ModuleInstance { .. } => 1,
        SubjectReassociation::InPort { .. } => 2,
        SubjectReassociation::OutPort { .. } => 3,
        SubjectReassociation::ExternalInput { .. } => 4,
        SubjectReassociation::ExternalOutput { .. } => 5,
        SubjectReassociation::__Domain(_, never) => match *never {},
    };
    (family, source, target)
}

/// Primary structural continuity of one subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Continuity {
    /// The same key survives with a compatible definition.
    Preserved,
    /// The same key survives with a replaced definition.
    ReplacedInPlace,
    /// An explicit reassociation carries the subject.
    Reassociated,
    /// The base subject has no successor.
    Removed,
    /// The target subject has no predecessor.
    Added,
}

/// One arm of a state predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConditionalArm {
    /// Keep the source state.
    Preserve,
    /// Carry the source state through a converting rule.
    Migrate,
    /// Install target declared state.
    Reset,
    /// Finalization rejects this arm.
    Reject,
}

/// Static state rule for one surviving stateful subject.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StateCompatibility {
    /// Source state survives unchanged.
    Preserve,
    /// Source state survives through a converting rule.
    Migrate,
    /// Target declared state replaces source state.
    Reset,
    /// The built-in rule rejects the pair.
    Reject,
    /// The target owns state the source did not.
    Initialize,
    /// Both arms stay observable. Preparation does not choose between them.
    Conditional {
        /// Runtime fact that selects the arm.
        fact: &'static str,
        /// Arm used when the fact is absent or Low.
        when_clear: ConditionalArm,
        /// Arm used when the fact is present or High.
        when_set: ConditionalArm,
    },
}

/// One total pending-work arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PendingArm {
    /// Keep the existing deadline.
    PreserveDeadline,
    /// Compute a new deadline from retained origin information.
    RecomputeDeadline,
    /// Change the payload while keeping the obligation.
    TransformPayload,
    /// Drop the obligation.
    Cancel,
    /// Finalization rejects this arm.
    Reject,
}

/// Static pending-work rule for one temporal owner.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PendingWorkRule {
    /// Keep existing deadlines.
    PreserveDeadline,
    /// Compute new deadlines from retained origins.
    RecomputeDeadline,
    /// Change payloads while keeping obligations.
    TransformPayload,
    /// Drop pending obligations.
    Cancel,
    /// Finalization rejects the pending obligation.
    Reject,
    /// Both arms stay observable. Preparation does not read the deciding fact.
    Conditional {
        /// Runtime fact that selects the arm.
        fact: &'static str,
        /// Arm used when the candidate, anchor, or schedule is absent.
        when_clear: PendingArm,
        /// Arm used when that fact is present.
        when_set: PendingArm,
    },
}

/// Planned fate of a diagnostic episode owned by a subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EpisodeRule {
    /// Keep the episode when its code and condition still match.
    Preserve,
    /// Rewrite the episode against the successor subject.
    Transform,
    /// Close the episode as resolved.
    Resolve,
    /// End the episode because its owner or state disappeared.
    Terminate,
    /// Finalization rejects the episode.
    Reject,
}

/// Planned fate of provenance for one subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProvenanceRule {
    /// Preserved state is a migration checkpoint.
    Checkpoint,
    /// Reset replaces the provenance root.
    Reset,
    /// An explicit retiming keeps the obligation and records the patch.
    Retime,
    /// Required ancestry ends.
    Loss,
}

/// Whether a classified loss is certain or state-dependent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LossClass {
    /// Every machine over the base topology incurs the loss.
    Unavoidable,
    /// The loss depends on a named runtime fact.
    Conditional,
}

/// One potential semantic loss. Entries are not collapsed by prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotentialSemanticLoss {
    class: LossClass,
    subject: SubjectRef,
    fact: &'static str,
    rule: &'static str,
}

impl PotentialSemanticLoss {
    /// Returns whether the loss is certain or conditional.
    #[must_use]
    pub const fn class(&self) -> LossClass {
        self.class
    }

    /// Returns the source subject.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the state or event fact.
    #[must_use]
    pub const fn fact(&self) -> &'static str {
        self.fact
    }

    /// Returns the migration rule or removal that causes the loss.
    #[must_use]
    pub const fn rule(&self) -> &'static str {
        self.rule
    }
}

/// Structural plan for one subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectPlan<D> {
    continuity: Continuity,
    metadata_changed: bool,
    hierarchy_changed: bool,
    source: Option<SubjectRef>,
    target: Option<SubjectRef>,
    state: Option<StateCompatibility>,
    directive: Option<NodeMigrationDirective<D>>,
}

impl<D> SubjectPlan<D> {
    /// Returns the primary continuity outcome.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns whether diagnostic metadata changed.
    #[must_use]
    pub const fn metadata_changed(&self) -> bool {
        self.metadata_changed
    }

    /// Returns whether hierarchy changed.
    #[must_use]
    pub const fn hierarchy_changed(&self) -> bool {
        self.hierarchy_changed
    }

    /// Returns the base subject when one exists.
    #[must_use]
    pub const fn source(&self) -> Option<&SubjectRef> {
        self.source.as_ref()
    }

    /// Returns the target subject when one exists.
    #[must_use]
    pub const fn target(&self) -> Option<&SubjectRef> {
        self.target.as_ref()
    }

    /// Returns the state rule for a surviving stateful subject.
    #[must_use]
    pub const fn state(&self) -> Option<&StateCompatibility> {
        self.state.as_ref()
    }

    /// Returns the node directive when the patch supplied one.
    #[must_use]
    pub const fn directive(&self) -> Option<NodeMigrationDirective<D>> {
        self.directive
    }
}

/// Pending-work rule for one temporal subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRule {
    subject: SubjectRef,
    predecessor: Option<SubjectRef>,
    rule: PendingWorkRule,
}

impl EventRule {
    /// Returns the temporal subject the rule applies to.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the base subject when the rule migrates an existing owner.
    #[must_use]
    pub const fn predecessor(&self) -> Option<&SubjectRef> {
        self.predecessor.as_ref()
    }

    /// Returns the total pending-work rule.
    #[must_use]
    pub const fn rule(&self) -> &PendingWorkRule {
        &self.rule
    }
}

/// Valuation plan for one external input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InputValuationPlan {
    /// A preserved level input keeps its valuation.
    Preserve,
    /// A reassociated level input inherits the source valuation.
    Inherit,
    /// A new level input must be established and is not defaulted.
    Establish,
    /// The input leaves the schema.
    Remove,
}

/// One external-input consequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalInputPlan {
    continuity: Continuity,
    source: Option<AnyExternalInputKey>,
    target: Option<AnyExternalInputKey>,
    valuation: InputValuationPlan,
}

impl ExternalInputPlan {
    /// Returns the input's continuity.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns the base input.
    #[must_use]
    pub const fn source(&self) -> Option<AnyExternalInputKey> {
        self.source
    }

    /// Returns the target input.
    #[must_use]
    pub const fn target(&self) -> Option<AnyExternalInputKey> {
        self.target
    }

    /// Returns the valuation consequence.
    #[must_use]
    pub const fn valuation(&self) -> InputValuationPlan {
        self.valuation
    }
}

/// Baseline plan for one external output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OutputBaselinePlan {
    /// A preserved level output keeps its baseline.
    Preserve,
    /// A reassociated level output carries the source baseline as evidence only.
    CarryAsEvidence,
    /// A new level output has no baseline.
    Establish,
    /// The output leaves the topology.
    Remove,
}

/// One external-output consequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalOutputPlan {
    continuity: Continuity,
    source: Option<AnyExternalOutputKey>,
    target: Option<AnyExternalOutputKey>,
    baseline: OutputBaselinePlan,
    level: bool,
}

impl ExternalOutputPlan {
    /// Returns the output's continuity.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns the base output.
    #[must_use]
    pub const fn source(&self) -> Option<AnyExternalOutputKey> {
        self.source
    }

    /// Returns the target output.
    #[must_use]
    pub const fn target(&self) -> Option<AnyExternalOutputKey> {
        self.target
    }

    /// Returns the baseline consequence.
    #[must_use]
    pub const fn baseline(&self) -> OutputBaselinePlan {
        self.baseline
    }

    /// Returns whether the output carries a level signal.
    #[must_use]
    pub const fn is_level(&self) -> bool {
        self.level
    }
}

/// One internal subject inside a module instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalSubjectPlan {
    role: String,
    continuity: Continuity,
    source: Option<SubjectRef>,
    target: Option<SubjectRef>,
    state: Option<StateCompatibility>,
}

impl InternalSubjectPlan {
    /// Returns the stable role or internal key label.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Returns the internal continuity.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns the base internal subject.
    #[must_use]
    pub const fn source(&self) -> Option<&SubjectRef> {
        self.source.as_ref()
    }

    /// Returns the target internal subject.
    #[must_use]
    pub const fn target(&self) -> Option<&SubjectRef> {
        self.target.as_ref()
    }

    /// Returns the internal state rule.
    #[must_use]
    pub const fn state(&self) -> Option<&StateCompatibility> {
        self.state.as_ref()
    }
}

/// Module-level continuity and internal correspondence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleContinuity<D> {
    continuity: Continuity,
    source: Option<ModuleInstanceKey>,
    target: Option<ModuleInstanceKey>,
    internals: Vec<InternalSubjectPlan>,
    directive: ModuleMigrationDirective<D>,
}

impl<D> ModuleContinuity<D> {
    /// Returns the instance continuity.
    #[must_use]
    pub const fn continuity(&self) -> Continuity {
        self.continuity
    }

    /// Returns the base instance.
    #[must_use]
    pub const fn source(&self) -> Option<ModuleInstanceKey> {
        self.source
    }

    /// Returns the target instance.
    #[must_use]
    pub const fn target(&self) -> Option<ModuleInstanceKey> {
        self.target
    }

    /// Returns internal correspondence in role order.
    #[must_use]
    pub fn internals(&self) -> &[InternalSubjectPlan] {
        &self.internals
    }

    /// Returns the module directive.
    #[must_use]
    pub const fn directive(&self) -> &ModuleMigrationDirective<D> {
        &self.directive
    }
}

/// How an endpoint change affects caller-owned bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EndpointChange {
    /// The endpoint was added.
    Added,
    /// The endpoint was removed.
    Removed,
    /// The endpoint was explicitly reassociated.
    Reassociated,
}

/// One endpoint change a caller may need when rebinding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointRebinding {
    change: EndpointChange,
    source: Option<SubjectRef>,
    target: Option<SubjectRef>,
}

impl EndpointRebinding {
    /// Returns the kind of endpoint change.
    #[must_use]
    pub const fn change(&self) -> EndpointChange {
        self.change
    }

    /// Returns the base endpoint.
    #[must_use]
    pub const fn source(&self) -> Option<&SubjectRef> {
        self.source.as_ref()
    }

    /// Returns the target endpoint.
    #[must_use]
    pub const fn target(&self) -> Option<&SubjectRef> {
        self.target.as_ref()
    }
}

/// A revision-bound artifact category made stale by commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ArtifactInvalidation {
    /// Resolved node, port, and endpoint handles.
    ResolvedHandles,
    /// Compiled inspection plans.
    CompiledInspectionPlans,
    /// Prepared patches bound to the old revision.
    OldRevisionPreparedPatches,
    /// Binding projectors bound to the old input schema.
    SchemaBoundBindingProjectors,
    /// Input snapshots and deltas bound to the old schema.
    OldSchemaInputArtifacts,
}

/// Whether a weak component merged or split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RegionChangeKind {
    /// Several base components meet in one target component.
    Merge,
    /// One base component occupies several target components.
    Split,
}

/// One derived region merge or split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionChange {
    kind: RegionChangeKind,
    from: Vec<SubjectRef>,
    into: Vec<SubjectRef>,
}

impl RegionChange {
    /// Returns whether the components merged or split.
    #[must_use]
    pub const fn kind(&self) -> RegionChangeKind {
        self.kind
    }

    /// Returns the base component identities involved.
    #[must_use]
    pub fn from(&self) -> &[SubjectRef] {
        &self.from
    }

    /// Returns the target component identities involved.
    #[must_use]
    pub fn into(&self) -> &[SubjectRef] {
        &self.into
    }
}

/// One diagnostic-episode plan entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodePlan {
    subject: SubjectRef,
    rule: EpisodeRule,
}

impl EpisodePlan {
    /// Returns the episode owner.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the episode rule.
    #[must_use]
    pub const fn rule(&self) -> EpisodeRule {
        self.rule
    }
}

/// One provenance plan entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenancePlan {
    subject: SubjectRef,
    rule: ProvenanceRule,
}

impl ProvenancePlan {
    /// Returns the provenance subject.
    #[must_use]
    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Returns the provenance rule.
    #[must_use]
    pub const fn rule(&self) -> ProvenanceRule {
        self.rule
    }
}

/// State-independent migration program for one prepared patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticMigrationPlan<D> {
    subjects: Vec<SubjectPlan<D>>,
    event_rules: Vec<EventRule>,
    external_inputs: Vec<ExternalInputPlan>,
    external_outputs: Vec<ExternalOutputPlan>,
    modules: Vec<ModuleContinuity<D>>,
    episodes: Vec<EpisodePlan>,
    provenance: Vec<ProvenancePlan>,
    potential_losses: Vec<PotentialSemanticLoss>,
    invalidated: Vec<ArtifactInvalidation>,
    region_changes: Vec<RegionChange>,
    rebinding: Vec<EndpointRebinding>,
}

impl<D> StaticMigrationPlan<D> {
    /// Returns structural subject records in canonical order.
    #[must_use]
    pub fn subjects(&self) -> &[SubjectPlan<D>] {
        &self.subjects
    }

    /// Returns pending-work rules, including rules for an empty schedule.
    #[must_use]
    pub fn event_rules(&self) -> &[EventRule] {
        &self.event_rules
    }

    /// Returns external-input consequences.
    #[must_use]
    pub fn external_inputs(&self) -> &[ExternalInputPlan] {
        &self.external_inputs
    }

    /// Returns external-output consequences.
    #[must_use]
    pub fn external_outputs(&self) -> &[ExternalOutputPlan] {
        &self.external_outputs
    }

    /// Returns module continuity, including internal correspondence.
    #[must_use]
    pub fn modules(&self) -> &[ModuleContinuity<D>] {
        &self.modules
    }

    /// Returns diagnostic-episode rules.
    #[must_use]
    pub fn episodes(&self) -> &[EpisodePlan] {
        &self.episodes
    }

    /// Returns provenance rules.
    #[must_use]
    pub fn provenance(&self) -> &[ProvenancePlan] {
        &self.provenance
    }

    /// Returns potential semantic losses.
    #[must_use]
    pub fn potential_losses(&self) -> &[PotentialSemanticLoss] {
        &self.potential_losses
    }

    /// Returns stale artifact categories. Invalidation is not semantic loss.
    #[must_use]
    pub fn invalidated(&self) -> &[ArtifactInvalidation] {
        &self.invalidated
    }

    /// Returns derived region merges and splits.
    #[must_use]
    pub fn region_changes(&self) -> &[RegionChange] {
        &self.region_changes
    }

    /// Returns endpoint changes relevant to caller-owned bindings.
    #[must_use]
    pub fn rebinding(&self) -> &[EndpointRebinding] {
        &self.rebinding
    }
}

struct PreparedInner<D> {
    network_key: NetworkKey,
    base_revision: NetworkRevision,
    proposed_revision: NetworkRevision,
    base_fingerprint: NetworkFingerprint,
    resulting_fingerprint: NetworkFingerprint,
    compiled: CompiledNetwork<D>,
    operations: Vec<PatchOperation<D>>,
    plan: StaticMigrationPlan<D>,
}

/// Immutable prepared replacement and its static migration plan.
///
/// Cloning shares the plan and compiled target. It does not require `D: Clone`.
pub struct PreparedPatch<D> {
    inner: Arc<PreparedInner<D>>,
}

impl<D> Clone for PreparedPatch<D> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<D: PartialEq> PartialEq for PreparedPatch<D> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.network_key == other.inner.network_key
            && self.inner.base_revision == other.inner.base_revision
            && self.inner.proposed_revision == other.inner.proposed_revision
            && self.inner.base_fingerprint == other.inner.base_fingerprint
            && self.inner.resulting_fingerprint == other.inner.resulting_fingerprint
            && self.inner.operations == other.inner.operations
            && self.inner.plan == other.inner.plan
    }
}

impl<D: PartialEq> Eq for PreparedPatch<D> {}

impl<D> fmt::Debug for PreparedPatch<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedPatch")
            .field("network_key", &self.inner.network_key)
            .field("base_revision", &self.inner.base_revision)
            .field("proposed_revision", &self.inner.proposed_revision)
            .field("base_fingerprint", &self.inner.base_fingerprint)
            .field("resulting_fingerprint", &self.inner.resulting_fingerprint)
            .finish_non_exhaustive()
    }
}

impl<D> PreparedPatch<D> {
    /// Returns the base network key.
    #[must_use]
    pub fn network_key(&self) -> NetworkKey {
        self.inner.network_key
    }

    /// Returns the base revision the patch was prepared against.
    #[must_use]
    pub fn base_revision(&self) -> NetworkRevision {
        self.inner.base_revision
    }

    /// Returns the uninstalled revision a later commit would install.
    #[must_use]
    pub fn proposed_revision(&self) -> NetworkRevision {
        self.inner.proposed_revision
    }

    /// Returns the base fingerprint.
    #[must_use]
    pub fn base_fingerprint(&self) -> NetworkFingerprint {
        self.inner.base_fingerprint
    }

    /// Returns the compiled target fingerprint.
    #[must_use]
    pub fn resulting_fingerprint(&self) -> NetworkFingerprint {
        self.inner.resulting_fingerprint
    }

    /// Returns the compiled target topology.
    #[must_use]
    pub fn resulting_compiled(&self) -> &CompiledNetwork<D> {
        &self.inner.compiled
    }

    /// Returns the normalized operations.
    #[must_use]
    pub fn operations(&self) -> PatchOperationIter<'_, D> {
        PatchOperationIter {
            inner: self.inner.operations.iter(),
        }
    }

    /// Returns the static migration plan.
    #[must_use]
    pub fn static_plan(&self) -> &StaticMigrationPlan<D> {
        &self.inner.plan
    }

    /// Starts a snapshot bound to the target input schema.
    #[must_use]
    pub fn input_snapshot(&self) -> crate::input::InputSnapshotBuilder<D> {
        self.inner.compiled.input_snapshot()
    }

    /// Starts a delta bound to the target input schema.
    #[must_use]
    pub fn input_delta(&self) -> crate::input::InputDeltaBuilder<D> {
        self.inner.compiled.input_delta()
    }
}

pub(crate) fn revision_mismatch<D: PartialEq>(
    network: NetworkKey,
    patch_revision: NetworkRevision,
    machine_revision: NetworkRevision,
) -> Report<PreparedPatch<D>, D> {
    let mut diagnostics = DiagnosticSet::new();
    push_problem(
        &mut diagnostics,
        SubjectRef::Network(network),
        Vec::new(),
        ProblemEvidence::ReconfigurationBaseRevisionMismatch {
            expected: machine_revision,
            actual: patch_revision,
            marker: PhantomData,
        },
    );
    Report::new(None, diagnostics)
}

pub(crate) fn prepare<D: PartialEq>(
    base: &CompiledNetwork<D>,
    patch: NetworkPatch<D>,
) -> Report<PreparedPatch<D>, D> {
    // SPEC: docs/specs/contracts/structural-preparation.yaml "no-machine-mutation"
    // Preparation reads the compiled definition and the patch, never runtime state.
    let mut diagnostics = DiagnosticSet::new();
    if patch.network_key != base.network_key()
        || patch.fingerprint != base.fingerprint()
        || patch.time_domain_id != base.time_domain_id()
    {
        push_problem(
            &mut diagnostics,
            SubjectRef::Network(base.network_key()),
            Vec::new(),
            ProblemEvidence::ReconfigurationBaseFingerprintMismatch {
                marker: PhantomData,
            },
        );
        return Report::new(None, diagnostics);
    }

    let definition = base.definition();
    let rewritten = rewrite(definition, &patch.operations, &mut diagnostics);
    check_reassociations(
        definition,
        &rewritten.definition,
        &patch.operations,
        &mut diagnostics,
    );
    let effective = rewritten.definition != *definition
        || rewritten.nonstandard
        || rewritten.node_parent
        || rewritten.port_meta
        || rewritten.reassociation;
    if !diagnostics.has_severity(crate::diagnostics::Severity::Error) && !effective {
        push_problem(
            &mut diagnostics,
            SubjectRef::Network(base.network_key()),
            Vec::new(),
            ProblemEvidence::ReconfigurationEmptyPatch {
                marker: PhantomData,
            },
        );
        return Report::new(None, diagnostics);
    }

    let validated = rewritten.definition.validate();
    let (validated, validation_diagnostics) = validated.into_parts();
    for diagnostic in validation_diagnostics {
        diagnostics.insert(diagnostic);
    }
    let Some(validated) = validated else {
        return Report::new(None, diagnostics);
    };
    if diagnostics.has_severity(crate::diagnostics::Severity::Error) {
        return Report::new(None, diagnostics);
    }
    let compiled = validated.compile();
    let (compiled, compile_diagnostics) = compiled.into_parts();
    for diagnostic in compile_diagnostics {
        diagnostics.insert(diagnostic);
    }
    let Some(compiled) = compiled else {
        return Report::new(None, diagnostics);
    };

    let plan = classify(
        definition,
        compiled.definition(),
        &patch.operations,
        base.input_schema_fingerprint(),
        compiled.input_schema_fingerprint(),
        &mut diagnostics,
    );
    let Some(proposed) = patch
        .base_revision
        .value()
        .checked_add(1)
        .map(NetworkRevision::from_value)
    else {
        // No catalogue code names revision overflow. Omit the artifact.
        return Report::new(None, diagnostics);
    };
    let prepared = PreparedPatch {
        inner: Arc::new(PreparedInner {
            network_key: base.network_key(),
            base_revision: patch.base_revision,
            proposed_revision: proposed,
            base_fingerprint: base.fingerprint(),
            resulting_fingerprint: compiled.fingerprint(),
            compiled,
            operations: patch.operations,
            plan,
        }),
    };
    Report::new(Some(prepared), diagnostics)
}

struct Rewrite<D> {
    definition: UncheckedNetwork<D>,
    nonstandard: bool,
    node_parent: bool,
    port_meta: bool,
    reassociation: bool,
}

fn rewrite<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    diagnostics: &mut DiagnosticSet<D>,
) -> Rewrite<D> {
    let mut nodes = base.nodes().to_vec();
    let mut connections = base.connections().to_vec();
    let mut inputs = base.external_inputs().to_vec();
    let mut outputs = base.external_outputs().to_vec();
    let mut modules = base.module_instances().to_vec();
    let mut network_meta = base.meta().clone();
    let mut nonstandard = false;
    let mut node_parent = false;
    let mut port_meta = false;
    let mut reassociation = false;
    let protected = retained_standard_internals(base, operations);

    for operation in operations {
        match operation {
            PatchOperation::AddNode(node) => {
                if !reject_internal(
                    &protected,
                    SubjectRef::Node(node.key()),
                    &nodes,
                    &connections,
                    diagnostics,
                ) {
                    nodes.push(node.clone());
                }
            }
            PatchOperation::RemoveNode { node } => {
                if !reject_internal(
                    &protected,
                    SubjectRef::Node(*node),
                    &nodes,
                    &connections,
                    diagnostics,
                ) && !remove_where(&mut nodes, |item| item.key() == *node)
                {
                    unknown(diagnostics, SubjectRef::Node(*node));
                }
            }
            PatchOperation::ReplaceNode {
                node,
                replacement,
                migration,
            } => {
                if !reject_internal(
                    &protected,
                    SubjectRef::Node(*node),
                    &nodes,
                    &connections,
                    diagnostics,
                ) {
                    if !migration_is_standard(*migration) {
                        nonstandard = true;
                    }
                    if !replace_where(&mut nodes, |item| item.key() == *node, replacement.clone()) {
                        unknown(diagnostics, SubjectRef::Node(*node));
                    }
                }
            }
            PatchOperation::AddConnection(connection) => {
                if !reject_internal(
                    &protected,
                    SubjectRef::Connection(connection.key()),
                    &nodes,
                    &connections,
                    diagnostics,
                ) {
                    connections.push(connection.clone());
                }
            }
            PatchOperation::RemoveConnection { connection } => {
                if !reject_internal(
                    &protected,
                    SubjectRef::Connection(*connection),
                    &nodes,
                    &connections,
                    diagnostics,
                ) && !remove_where(&mut connections, |item| item.key() == *connection)
                {
                    unknown(diagnostics, SubjectRef::Connection(*connection));
                }
            }
            PatchOperation::ReplaceConnection {
                connection,
                replacement,
            } => {
                if !reject_internal(
                    &protected,
                    SubjectRef::Connection(*connection),
                    &nodes,
                    &connections,
                    diagnostics,
                ) && !replace_where(
                    &mut connections,
                    |item| item.key() == *connection,
                    replacement.clone(),
                ) {
                    unknown(diagnostics, SubjectRef::Connection(*connection));
                }
            }
            PatchOperation::AddExternalInput(input) => inputs.push(input.clone()),
            PatchOperation::RemoveExternalInput { input } => {
                if !remove_where(&mut inputs, |item| item.key() == *input) {
                    unknown(diagnostics, SubjectRef::ExternalInput(*input));
                }
            }
            PatchOperation::ReplaceExternalInput { input, replacement } => {
                if !replace_where(
                    &mut inputs,
                    |item| item.key() == *input,
                    replacement.clone(),
                ) {
                    unknown(diagnostics, SubjectRef::ExternalInput(*input));
                }
            }
            PatchOperation::AddExternalOutput(output) => outputs.push(output.clone()),
            PatchOperation::RemoveExternalOutput { output } => {
                if !remove_where(&mut outputs, |item| item.key() == *output) {
                    unknown(diagnostics, SubjectRef::ExternalOutput(*output));
                }
            }
            PatchOperation::ReplaceExternalOutput {
                output,
                replacement,
            } => {
                if !replace_where(
                    &mut outputs,
                    |item| item.key() == *output,
                    replacement.clone(),
                ) {
                    unknown(diagnostics, SubjectRef::ExternalOutput(*output));
                }
            }
            PatchOperation::AddModuleInstance(instance) => modules.push(instance.clone()),
            PatchOperation::RemoveModuleInstance { module } => {
                if !remove_where(&mut modules, |item| item.key() == *module) {
                    unknown(diagnostics, SubjectRef::ModuleInstance(*module));
                }
            }
            PatchOperation::ReplaceModuleInstance {
                module,
                replacement,
                migration,
            } => {
                if !module_directive_is_standard(migration) {
                    nonstandard = true;
                }
                if !replace_where(
                    &mut modules,
                    |item| item.key() == *module,
                    replacement.clone(),
                ) {
                    unknown(diagnostics, SubjectRef::ModuleInstance(*module));
                }
            }
            PatchOperation::SetParent { subject, parent } => match subject {
                HierarchicalSubjectRef::ModuleInstance(module) => {
                    if !update_where(
                        &mut modules,
                        |item| item.key() == *module,
                        |item| {
                            *item = module_with_parent(item.clone(), *parent);
                        },
                    ) {
                        unknown(diagnostics, SubjectRef::ModuleInstance(*module));
                    }
                }
                HierarchicalSubjectRef::Node(node) => {
                    if !nodes.iter().any(|item| item.key() == *node) {
                        unknown(diagnostics, SubjectRef::Node(*node));
                    } else if parent.is_some() {
                        node_parent = true;
                    }
                }
            },
            PatchOperation::SetDiagnosticMeta { subject, meta } => match subject {
                StructuralSubjectRef::Network(key) => {
                    if *key == base.key() {
                        if meta != &network_meta {
                            network_meta = meta.clone();
                        }
                    } else {
                        unknown(diagnostics, SubjectRef::Network(*key));
                    }
                }
                StructuralSubjectRef::Node(key) => {
                    if !update_where(
                        &mut nodes,
                        |item| item.key() == *key,
                        |item| {
                            if item.meta() != meta {
                                *item = node_with_meta(item.clone(), meta.clone());
                            }
                        },
                    ) {
                        unknown(diagnostics, SubjectRef::Node(*key));
                    }
                }
                StructuralSubjectRef::Connection(key) => {
                    if !update_where(
                        &mut connections,
                        |item| item.key() == *key,
                        |item| {
                            if item.meta() != meta {
                                *item = connection_with_meta(item.clone(), meta.clone());
                            }
                        },
                    ) {
                        unknown(diagnostics, SubjectRef::Connection(*key));
                    }
                }
                StructuralSubjectRef::ExternalInput(key) => {
                    if !update_where(
                        &mut inputs,
                        |item| item.key() == *key,
                        |item| {
                            if item.meta() != meta {
                                *item = input_with_meta(item.clone(), meta.clone());
                            }
                        },
                    ) {
                        unknown(diagnostics, SubjectRef::ExternalInput(*key));
                    }
                }
                StructuralSubjectRef::ExternalOutput(key) => {
                    if !update_where(
                        &mut outputs,
                        |item| item.key() == *key,
                        |item| {
                            if item.meta() != meta {
                                *item = output_with_meta(item.clone(), meta.clone());
                            }
                        },
                    ) {
                        unknown(diagnostics, SubjectRef::ExternalOutput(*key));
                    }
                }
                StructuralSubjectRef::Module(key) => {
                    if !update_where(
                        &mut modules,
                        |item| item.key() == *key,
                        |item| {
                            if item.meta() != meta {
                                *item = module_with_meta(item.clone(), meta.clone());
                            }
                        },
                    ) {
                        unknown(diagnostics, SubjectRef::ModuleInstance(*key));
                    }
                }
                StructuralSubjectRef::InPort(key) => {
                    if nodes.iter().any(|node| node.ports().inputs().contains(key)) {
                        port_meta = true;
                    } else {
                        unknown(diagnostics, SubjectRef::InPort(*key));
                    }
                }
                StructuralSubjectRef::OutPort(key) => {
                    if nodes
                        .iter()
                        .any(|node| node.ports().outputs().contains(key))
                    {
                        port_meta = true;
                    } else {
                        unknown(diagnostics, SubjectRef::OutPort(*key));
                    }
                }
            },
            PatchOperation::Reassociate(mapping) => {
                reassociation = true;
                if !reassoc_directive_is_standard(mapping) {
                    nonstandard = true;
                }
            }
        }
    }

    nodes.sort_by_key(|node| node.key());
    connections.sort_by_key(|connection| connection.key());
    inputs.sort_by_key(|input| input.key());
    outputs.sort_by_key(|output| output.key());
    modules.sort_by_key(|module| module.key());
    Rewrite {
        definition: UncheckedNetwork::new_with_instances(
            base.key(),
            base.time_domain_id(),
            network_meta,
            nodes,
            inputs,
            outputs,
            connections,
            modules,
        ),
        nonstandard,
        node_parent,
        port_meta,
        reassociation,
    }
}

fn migration_is_standard<D>(directive: NodeMigrationDirective<D>) -> bool {
    matches!(directive, NodeMigrationDirective::Standard)
}

fn module_directive_is_standard<D>(directive: &ModuleMigrationDirective<D>) -> bool {
    matches!(directive, ModuleMigrationDirective::Standard)
}

fn reassoc_directive_is_standard<D>(mapping: &SubjectReassociation<D>) -> bool {
    match mapping {
        SubjectReassociation::Node { migration, .. } => migration_is_standard(*migration),
        SubjectReassociation::ModuleInstance { migration, .. } => {
            module_directive_is_standard(migration)
        }
        _ => true,
    }
}

fn remove_where<T>(items: &mut Vec<T>, predicate: impl Fn(&T) -> bool) -> bool {
    let before = items.len();
    items.retain(|item| !predicate(item));
    items.len() != before
}

fn replace_where<T: Clone>(
    items: &mut [T],
    predicate: impl Fn(&T) -> bool,
    replacement: T,
) -> bool {
    let Some(index) = items.iter().position(predicate) else {
        return false;
    };
    items[index] = replacement;
    true
}

fn update_where<T>(
    items: &mut [T],
    predicate: impl Fn(&T) -> bool,
    update: impl FnOnce(&mut T),
) -> bool {
    let Some(item) = items.iter_mut().find(|item| predicate(item)) else {
        return false;
    };
    update(item);
    true
}

fn unknown<D: PartialEq>(diagnostics: &mut DiagnosticSet<D>, subject: SubjectRef) {
    push_problem(
        diagnostics,
        subject,
        Vec::new(),
        ProblemEvidence::ReconfigurationUnknownBaseSubject {
            marker: PhantomData,
        },
    );
}

fn retained_standard_internals<D>(
    base: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
) -> Vec<(ModuleInstanceKey, SubjectRef)> {
    let replaced = operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::RemoveModuleInstance { module }
            | PatchOperation::ReplaceModuleInstance { module, .. } => Some(*module),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut found = Vec::new();
    for instance in base.module_instances() {
        if replaced.contains(&instance.key()) {
            continue;
        }
        if instance.module().standard_declaration().is_none() {
            continue;
        }
        for node in instance.module().graph().nodes() {
            found.push((instance.key(), SubjectRef::Node(node.key())));
        }
        for connection in instance.module().graph().connections() {
            found.push((instance.key(), SubjectRef::Connection(connection.key())));
        }
    }
    found
}

fn reject_internal<D: PartialEq>(
    protected: &[(ModuleInstanceKey, SubjectRef)],
    subject: SubjectRef,
    nodes: &[NodeDef<D>],
    connections: &[ConnectionDef],
    diagnostics: &mut DiagnosticSet<D>,
) -> bool {
    let top_level = match subject {
        SubjectRef::Node(key) => nodes.iter().any(|node| node.key() == key),
        SubjectRef::Connection(key) => connections.iter().any(|connection| connection.key() == key),
        _ => return false,
    };
    if top_level {
        return false;
    }
    let Some((instance, _)) = protected
        .iter()
        .find(|(_, protected)| protected == &subject)
    else {
        return false;
    };
    push_problem(
        diagnostics,
        SubjectRef::ModuleInstance(*instance),
        vec![RelatedSubject {
            role: RelatedSubjectRole::TargetSubject,
            subject: subject.clone(),
        }],
        ProblemEvidence::StandardModuleNoncanonicalInternalEdit {
            instance: *instance,
            subject,
            marker: PhantomData,
        },
    );
    true
}

fn check_reassociations<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    diagnostics: &mut DiagnosticSet<D>,
) {
    let mut sources = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for operation in operations {
        let PatchOperation::Reassociate(mapping) = operation else {
            continue;
        };
        let (source, destination) = reassoc_ends(mapping);
        if !sources.insert(source.clone()) || !targets.insert(destination.clone()) {
            push_problem(
                diagnostics,
                source.clone(),
                vec![RelatedSubject {
                    role: RelatedSubjectRole::MigrationTarget,
                    subject: destination.clone(),
                }],
                ProblemEvidence::ReconfigurationNonInjectiveReassociation {
                    marker: PhantomData,
                },
            );
            continue;
        }
        let source_ok = subject_present(base, &source) && !subject_present(target, &source);
        let target_ok =
            subject_present(target, &destination) && !subject_present(base, &destination);
        let kinds_ok = reassoc_structure_ok(base, target, mapping);
        let schema_ok = !matches!(
            mapping,
            SubjectReassociation::ExternalInput { from, to } if from.kind() != to.kind()
        );
        if !schema_ok {
            push_problem(
                diagnostics,
                source.clone(),
                Vec::new(),
                ProblemEvidence::ReconfigurationInvalidTargetInputSchema {
                    marker: PhantomData,
                },
            );
        }
        if !source_ok || !target_ok || !kinds_ok {
            push_problem(
                diagnostics,
                source,
                vec![RelatedSubject {
                    role: RelatedSubjectRole::MigrationTarget,
                    subject: destination,
                }],
                ProblemEvidence::ReconfigurationInvalidReassociation {
                    source: reassoc_ends(mapping).0,
                    target: reassoc_ends(mapping).1,
                    marker: PhantomData,
                },
            );
        }
    }
}

fn subject_present<D>(definition: &UncheckedNetwork<D>, subject: &SubjectRef) -> bool {
    match subject {
        SubjectRef::Node(key) => definition.nodes().iter().any(|node| node.key() == *key),
        SubjectRef::Connection(key) => definition
            .connections()
            .iter()
            .any(|connection| connection.key() == *key),
        SubjectRef::ExternalInput(key) => definition
            .external_inputs()
            .iter()
            .any(|input| input.key() == *key),
        SubjectRef::ExternalOutput(key) => definition
            .external_outputs()
            .iter()
            .any(|output| output.key() == *key),
        SubjectRef::ModuleInstance(key) => definition
            .module_instances()
            .iter()
            .any(|instance| instance.key() == *key),
        SubjectRef::InPort(key) => definition
            .nodes()
            .iter()
            .any(|node| node.ports().inputs().contains(key)),
        SubjectRef::OutPort(key) => definition
            .nodes()
            .iter()
            .any(|node| node.ports().outputs().contains(key)),
        SubjectRef::Network(key) => definition.key() == *key,
        _ => false,
    }
}

fn reassoc_structure_ok<D>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    mapping: &SubjectReassociation<D>,
) -> bool {
    match mapping {
        SubjectReassociation::Node { from, to, .. } => {
            match (find_node(base, *from), find_node(target, *to)) {
                (Some(source), Some(destination)) => ports_compatible(source, destination),
                _ => false,
            }
        }
        SubjectReassociation::ExternalInput { from, to } => from.kind() == to.kind(),
        SubjectReassociation::ExternalOutput { from, to } => from.kind() == to.kind(),
        SubjectReassociation::InPort { from, to } => from.kind() == to.kind(),
        SubjectReassociation::OutPort { from, to } => from.kind() == to.kind(),
        SubjectReassociation::ModuleInstance { .. } => true,
        SubjectReassociation::__Domain(_, never) => match *never {},
    }
}

fn find_node<D>(definition: &UncheckedNetwork<D>, key: NodeKey) -> Option<&NodeDef<D>> {
    definition.nodes().iter().find(|node| node.key() == key)
}

fn ports_compatible<D>(source: &NodeDef<D>, target: &NodeDef<D>) -> bool {
    let (source_inputs, source_outputs) = port_kinds(source);
    let (target_inputs, target_outputs) = port_kinds(target);
    if node_schema(source.kind()).is_variadic() || node_schema(target.kind()).is_variadic() {
        return kind_multiset(&source_inputs) == kind_multiset(&target_inputs)
            && kind_multiset(&source_outputs) == kind_multiset(&target_outputs);
    }
    source.ports().input_roles() == target.ports().input_roles()
        && source.ports().output_roles() == target.ports().output_roles()
        && source_inputs == target_inputs
        && source_outputs == target_outputs
}

fn port_kinds<D>(node: &NodeDef<D>) -> (Vec<SignalKind>, Vec<SignalKind>) {
    (
        node.ports()
            .inputs()
            .iter()
            .map(|port| port.kind())
            .collect(),
        node.ports()
            .outputs()
            .iter()
            .map(|port| port.kind())
            .collect(),
    )
}

fn kind_multiset(kinds: &[SignalKind]) -> BTreeMap<SignalKind, usize> {
    let mut counts = BTreeMap::new();
    for kind in kinds {
        *counts.entry(*kind).or_default() += 1;
    }
    counts
}

fn push_problem<D: PartialEq>(
    diagnostics: &mut DiagnosticSet<D>,
    primary: SubjectRef,
    related: Vec<RelatedSubject>,
    evidence: ProblemEvidence<D>,
) {
    let problem = Problem::new(primary, related, evidence);
    match Diagnostic::new(problem) {
        Ok(diagnostic) => diagnostics.insert(diagnostic),
        Err(problem) => panic!(
            "preparation catalogue evidence must allow report delivery: {}",
            problem.code().as_str()
        ),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BooleanKind {
    Toggle,
    PulseLatch,
    LevelLatch,
    SampleHold,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TemporalKind {
    PulseDelay,
    Transport,
    Inertial,
    Periodic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Combinational,
    Edge(EdgeDetectorKind),
    Boolean(BooleanKind),
    Temporal(TemporalKind),
}

fn family_of<D>(kind: &NodeKind<D>) -> Family {
    match kind {
        NodeKind::Constant(_)
        | NodeKind::Not
        | NodeKind::All
        | NodeKind::Any
        | NodeKind::Parity
        | NodeKind::AtLeast(_)
        | NodeKind::Select
        | NodeKind::Merge
        | NodeKind::Coalesce
        | NodeKind::Zip
        | NodeKind::PulseGate
        | NodeKind::PulseSelect
        | NodeKind::PulseRoute => Family::Combinational,
        NodeKind::RisingEdge(_) => Family::Edge(EdgeDetectorKind::Rising),
        NodeKind::FallingEdge(_) => Family::Edge(EdgeDetectorKind::Falling),
        NodeKind::AnyEdge(_) => Family::Edge(EdgeDetectorKind::Any),
        NodeKind::Toggle(_) => Family::Boolean(BooleanKind::Toggle),
        NodeKind::PulseSetResetLatch(_) => Family::Boolean(BooleanKind::PulseLatch),
        NodeKind::LevelSetResetLatch(_) => Family::Boolean(BooleanKind::LevelLatch),
        NodeKind::SampleHold(_) => Family::Boolean(BooleanKind::SampleHold),
        NodeKind::PulseDelay(_) => Family::Temporal(TemporalKind::PulseDelay),
        NodeKind::TransportDelay(_) => Family::Temporal(TemporalKind::Transport),
        NodeKind::InertialDelay(_) => Family::Temporal(TemporalKind::Inertial),
        NodeKind::Periodic(_) => Family::Temporal(TemporalKind::Periodic),
    }
}

fn temporal_parameters_changed<D>(source: &NodeKind<D>, target: &NodeKind<D>) -> bool {
    match (source, target) {
        (NodeKind::PulseDelay(left), NodeKind::PulseDelay(right)) => left.delay != right.delay,
        (NodeKind::TransportDelay(left), NodeKind::TransportDelay(right)) => {
            left.delay != right.delay
        }
        (NodeKind::InertialDelay(left), NodeKind::InertialDelay(right)) => {
            left.delay != right.delay
        }
        (NodeKind::Periodic(left), NodeKind::Periodic(right)) => {
            left.period != right.period
                || left.first_emission != right.first_emission
                || left.reenable_phase != right.reenable_phase
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct LossDraft {
    class: LossClass,
    fact: &'static str,
    rule: &'static str,
}

struct Classified {
    state: Option<StateCompatibility>,
    pending: Option<PendingWorkRule>,
    losses: Vec<LossDraft>,
    episode: Option<EpisodeRule>,
    provenance: Option<ProvenanceRule>,
    error: Option<ClassifiedError>,
}

#[derive(Clone, Copy)]
enum ClassifiedError {
    Incompatible,
    CrossKind,
}

fn blank() -> Classified {
    Classified {
        state: None,
        pending: None,
        losses: Vec::new(),
        episode: None,
        provenance: None,
        error: None,
    }
}

fn loss(class: LossClass, fact: &'static str, rule: &'static str) -> LossDraft {
    LossDraft { class, fact, rule }
}

fn failed(error: ClassifiedError, temporal: bool) -> Classified {
    Classified {
        pending: temporal.then_some(PendingWorkRule::Reject),
        error: Some(error),
        ..blank()
    }
}

struct Acc<'a, D> {
    diagnostics: &'a mut DiagnosticSet<D>,
    subjects: &'a mut Vec<SubjectPlan<D>>,
    events: &'a mut Vec<EventRule>,
    losses: &'a mut Vec<PotentialSemanticLoss>,
    episodes: &'a mut Vec<EpisodePlan>,
    provenance: &'a mut Vec<ProvenancePlan>,
}

impl<D: PartialEq> Acc<'_, D> {
    fn error(&mut self, subject: SubjectRef, error: ClassifiedError) {
        let evidence = match error {
            ClassifiedError::Incompatible => {
                ProblemEvidence::ReconfigurationIncompatibleMigrationDirective {
                    marker: PhantomData,
                }
            }
            ClassifiedError::CrossKind => {
                ProblemEvidence::ReconfigurationUnsupportedCrossKindMigration {
                    marker: PhantomData,
                }
            }
        };
        push_problem(self.diagnostics, subject, Vec::new(), evidence);
    }

    fn loss(
        &mut self,
        class: LossClass,
        subject: SubjectRef,
        fact: &'static str,
        rule: &'static str,
    ) {
        let evidence = match class {
            LossClass::Unavoidable => ProblemEvidence::ReconfigurationUnavoidableSemanticLoss {
                fact,
                rule,
                marker: PhantomData,
            },
            LossClass::Conditional => ProblemEvidence::ReconfigurationConditionalSemanticLoss {
                fact,
                rule,
                marker: PhantomData,
            },
        };
        push_problem(self.diagnostics, subject.clone(), Vec::new(), evidence);
        self.losses.push(PotentialSemanticLoss {
            class,
            subject,
            fact,
            rule,
        });
    }

    fn commit(
        &mut self,
        event_subject: SubjectRef,
        predecessor: Option<SubjectRef>,
        loss_subject: SubjectRef,
        classified: &Classified,
    ) {
        if let Some(error) = classified.error {
            self.error(event_subject.clone(), error);
        }
        for draft in &classified.losses {
            self.loss(draft.class, loss_subject.clone(), draft.fact, draft.rule);
        }
        if let Some(rule) = classified.pending.clone() {
            self.events.push(EventRule {
                subject: event_subject,
                predecessor,
                rule,
            });
        }
        if let Some(rule) = classified.episode {
            self.episodes.push(EpisodePlan {
                subject: loss_subject.clone(),
                rule,
            });
        }
        if let Some(rule) = classified.provenance {
            self.provenance.push(ProvenancePlan {
                subject: loss_subject,
                rule,
            });
        }
    }
}

fn state_fact(family: StateFamily) -> &'static str {
    match family {
        StateFamily::EdgeObservation => "edge_observation",
        StateFamily::StoredLevel => "stored_level",
        StateFamily::TransportLevel => "transport_level",
        StateFamily::PeriodicEnable => "periodic_enable",
    }
}

fn pending_fact(family: TemporalFamily) -> &'static str {
    match family {
        TemporalFamily::PendingPulseGroup => "pending_group",
        TemporalFamily::PendingTransportTransition => "pending_transition",
        TemporalFamily::InertialCandidate => "inertial_candidate",
        TemporalFamily::PeriodicBoundary => "periodic_schedule",
    }
}

fn temporal_side<D>(kind: &NodeKind<D>) -> bool {
    node_schema(kind).temporal_family().is_some()
}

fn preserves(classified: &Classified) -> bool {
    let state_ok = match classified.state {
        None
        | Some(
            StateCompatibility::Preserve
            | StateCompatibility::Initialize
            | StateCompatibility::Conditional { .. },
        ) => true,
        Some(
            StateCompatibility::Migrate | StateCompatibility::Reset | StateCompatibility::Reject,
        ) => false,
    };
    let pending_ok = match classified.pending {
        None | Some(PendingWorkRule::PreserveDeadline | PendingWorkRule::Conditional { .. }) => {
            true
        }
        Some(
            PendingWorkRule::RecomputeDeadline
            | PendingWorkRule::TransformPayload
            | PendingWorkRule::Cancel
            | PendingWorkRule::Reject,
        ) => false,
    };
    state_ok && pending_ok
}

fn directive_fits<D>(directive: NodeMigrationDirective<D>, source: Family, target: Family) -> bool {
    match directive {
        NodeMigrationDirective::Standard
        | NodeMigrationDirective::RequirePreserve
        | NodeMigrationDirective::Reset => true,
        NodeMigrationDirective::TransferStoredLevel => {
            matches!(source, Family::Boolean(_)) && matches!(target, Family::Boolean(_))
        }
        NodeMigrationDirective::PulseDelay(_) => matches!(
            (source, target),
            (
                Family::Temporal(TemporalKind::PulseDelay),
                Family::Temporal(TemporalKind::PulseDelay)
            )
        ),
        NodeMigrationDirective::TransportDelay(_) => matches!(
            (source, target),
            (
                Family::Temporal(TemporalKind::Transport),
                Family::Temporal(TemporalKind::Transport)
            )
        ),
        NodeMigrationDirective::InertialDelay(_) => matches!(
            (source, target),
            (
                Family::Temporal(TemporalKind::Inertial),
                Family::Temporal(TemporalKind::Inertial)
            )
        ),
        NodeMigrationDirective::Periodic(_) => matches!(
            (source, target),
            (
                Family::Temporal(TemporalKind::Periodic),
                Family::Temporal(TemporalKind::Periodic)
            )
        ),
        NodeMigrationDirective::__Domain(_, never) => match never {},
    }
}

fn with_memory(fact: Option<&'static str>) -> Classified {
    let mut classified = blank();
    if fact.is_some() {
        classified.state = Some(StateCompatibility::Preserve);
        classified.episode = Some(EpisodeRule::Preserve);
    }
    classified.provenance = Some(ProvenanceRule::Checkpoint);
    classified
}

fn target_initialized<D>(target: &NodeKind<D>) -> Classified {
    let mut classified = blank();
    if node_schema(target).state_family().is_some() {
        classified.state = Some(StateCompatibility::Initialize);
    }
    if node_schema(target).temporal_family().is_some() {
        classified.pending = Some(PendingWorkRule::PreserveDeadline);
    }
    if classified.state.is_some() || classified.pending.is_some() {
        classified.provenance = Some(ProvenanceRule::Checkpoint);
    }
    classified
}

fn source_discarded<D>(source: &NodeKind<D>, rule_name: &'static str) -> Classified {
    let mut classified = blank();
    if let Some(family) = node_schema(source).state_family() {
        classified
            .losses
            .push(loss(LossClass::Unavoidable, state_fact(family), rule_name));
    }
    if let Some(family) = node_schema(source).temporal_family() {
        classified.pending = Some(PendingWorkRule::Cancel);
        classified.losses.push(loss(
            LossClass::Conditional,
            pending_fact(family),
            rule_name,
        ));
    }
    if node_schema(source).state_family().is_some()
        || node_schema(source).temporal_family().is_some()
    {
        classified.episode = Some(EpisodeRule::Terminate);
        classified.provenance = Some(ProvenanceRule::Loss);
    }
    classified
}

fn reset_owner<D>(source: &NodeKind<D>, target: &NodeKind<D>) -> Classified {
    let mut classified = blank();
    if node_schema(source).state_family().is_some() || node_schema(target).state_family().is_some()
    {
        classified.state = Some(StateCompatibility::Reset);
    }
    if let Some(family) = node_schema(source).state_family() {
        classified
            .losses
            .push(loss(LossClass::Unavoidable, state_fact(family), "reset"));
    }
    if node_schema(source).temporal_family().is_some()
        || node_schema(target).temporal_family().is_some()
    {
        classified.pending = Some(PendingWorkRule::Cancel);
    }
    if let Some(family) = node_schema(source).temporal_family() {
        classified
            .losses
            .push(loss(LossClass::Conditional, pending_fact(family), "reset"));
    }
    if node_schema(source).state_family().is_some()
        || node_schema(source).temporal_family().is_some()
    {
        classified.episode = Some(EpisodeRule::Terminate);
        classified.provenance = Some(ProvenanceRule::Reset);
    } else if classified.state.is_some() || classified.pending.is_some() {
        classified.provenance = Some(ProvenanceRule::Checkpoint);
    }
    classified
}

fn unrelated_reject<D>(source: &NodeKind<D>, target: &NodeKind<D>) -> Classified {
    let mut classified = blank();
    if node_schema(source).state_family().is_some() || node_schema(target).state_family().is_some()
    {
        classified.state = Some(StateCompatibility::Reject);
    }
    if temporal_side(source) || temporal_side(target) {
        classified.pending = Some(PendingWorkRule::Reject);
    }
    classified.episode = Some(EpisodeRule::Reject);
    classified.provenance = Some(ProvenanceRule::Checkpoint);
    classified
}

fn edge_rule(same_kind: bool) -> Classified {
    let mut classified = with_memory(Some("edge_observation"));
    classified.state = Some(if same_kind {
        StateCompatibility::Preserve
    } else {
        StateCompatibility::Migrate
    });
    classified
}

fn boolean_standard(same_kind: bool) -> Classified {
    if same_kind {
        let mut classified = with_memory(Some("stored_level"));
        classified.state = Some(StateCompatibility::Preserve);
        classified
    } else {
        let mut classified = blank();
        classified.state = Some(StateCompatibility::Reject);
        classified.episode = Some(EpisodeRule::Reject);
        classified.provenance = Some(ProvenanceRule::Checkpoint);
        classified
    }
}

fn boolean_transfer(same_kind: bool) -> Classified {
    let mut classified = with_memory(Some("stored_level"));
    classified.state = Some(if same_kind {
        StateCompatibility::Preserve
    } else {
        StateCompatibility::Migrate
    });
    classified
}

// SPEC: docs/specs/contracts/structural-preparation.yaml "total-temporal-rules"
// Standard inertial and periodic parameter changes stay conditional predicates.
fn temporal_standard<D>(
    source: &NodeKind<D>,
    target: &NodeKind<D>,
    kind: TemporalKind,
) -> Classified {
    let fact = node_schema(source).state_family().map(state_fact);
    let mut classified = with_memory(fact);
    let changed = temporal_parameters_changed(source, target);
    classified.pending = Some(match (kind, changed) {
        (TemporalKind::Inertial, true) => {
            classified.losses.push(loss(
                LossClass::Conditional,
                "inertial_candidate",
                "standard",
            ));
            PendingWorkRule::Conditional {
                fact: "inertial_candidate",
                when_clear: PendingArm::PreserveDeadline,
                when_set: PendingArm::Reject,
            }
        }
        (TemporalKind::Periodic, true) => {
            classified.losses.push(loss(
                LossClass::Conditional,
                "periodic_schedule",
                "standard",
            ));
            PendingWorkRule::Conditional {
                fact: "periodic_anchor",
                when_clear: PendingArm::RecomputeDeadline,
                when_set: PendingArm::Reject,
            }
        }
        _ => PendingWorkRule::PreserveDeadline,
    });
    classified
}

fn pulse_policy(policy: PulseDelayMigration) -> Classified {
    let mut classified = with_memory(None);
    classified.episode = None;
    match policy {
        PulseDelayMigration::PreserveDeadlines => {
            classified.pending = Some(PendingWorkRule::PreserveDeadline);
        }
        PulseDelayMigration::RecomputeFromOrigin { .. } => {
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        PulseDelayMigration::RestartFromPatchTime => {
            // SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md "48.3 `RestartFromPatchTime`"
            // Every group is retimed from patch time. Elapsed-wait loss stays conditional.
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.losses.push(loss(
                LossClass::Conditional,
                "elapsed_wait",
                "restart_from_patch_time",
            ));
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        PulseDelayMigration::CancelPending => {
            classified.pending = Some(PendingWorkRule::Cancel);
            classified
                .losses
                .push(loss(LossClass::Conditional, "pending_group", "cancel"));
        }
        PulseDelayMigration::RejectIfPending => {
            classified.pending = Some(PendingWorkRule::Conditional {
                fact: "pending_group",
                when_clear: PendingArm::PreserveDeadline,
                when_set: PendingArm::Reject,
            });
        }
    }
    classified
}

fn transport_policy(policy: TransportDelayMigration) -> Classified {
    let mut classified = with_memory(Some("transport_level"));
    match policy {
        TransportDelayMigration::PreserveDeadlines => {
            classified.pending = Some(PendingWorkRule::PreserveDeadline);
        }
        TransportDelayMigration::RecomputeFromOrigin { .. } => {
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        TransportDelayMigration::RestartFromPatchTime => {
            // SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md "49.2 Recomputed or restarted queues"
            // Restarted transitions are retimed. Elapsed-wait loss stays conditional.
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.losses.push(loss(
                LossClass::Conditional,
                "elapsed_wait",
                "restart_from_patch_time",
            ));
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        TransportDelayMigration::CancelPending => {
            classified.pending = Some(PendingWorkRule::Cancel);
            classified
                .losses
                .push(loss(LossClass::Conditional, "pending_transition", "cancel"));
        }
        TransportDelayMigration::RejectIfPending => {
            classified.pending = Some(PendingWorkRule::Conditional {
                fact: "pending_transition",
                when_clear: PendingArm::PreserveDeadline,
                when_set: PendingArm::Reject,
            });
        }
    }
    classified
}

fn inertial_policy(policy: InertialDelayMigration) -> Classified {
    let mut classified = with_memory(Some("transport_level"));
    match policy {
        InertialDelayMigration::PreserveDeadline => {
            classified.pending = Some(PendingWorkRule::PreserveDeadline);
        }
        InertialDelayMigration::RecomputeFromOrigin { .. } => {
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        InertialDelayMigration::RestartFromPatchTime => {
            // SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md "50.4 `RestartFromPatchTime`"
            // Qualification restarts at patch time. Elapsed qualification is conditional loss.
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.losses.push(loss(
                LossClass::Conditional,
                "elapsed_qualification",
                "restart_from_patch_time",
            ));
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        InertialDelayMigration::CancelCandidate => {
            classified.pending = Some(PendingWorkRule::Cancel);
            classified
                .losses
                .push(loss(LossClass::Conditional, "inertial_candidate", "cancel"));
        }
        InertialDelayMigration::RejectIfCandidate => {
            classified.pending = Some(PendingWorkRule::Conditional {
                fact: "inertial_candidate",
                when_clear: PendingArm::PreserveDeadline,
                when_set: PendingArm::Reject,
            });
        }
    }
    classified
}

fn periodic_policy(policy: PeriodicMigration) -> Classified {
    let mut classified = with_memory(Some("periodic_enable"));
    match policy {
        PeriodicMigration::PreserveNextDeadline => {
            classified.pending = Some(PendingWorkRule::PreserveDeadline);
        }
        PeriodicMigration::RecomputeFromExistingAnchor => {
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        PeriodicMigration::ReanchorAtPatchTime => {
            // SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md "51.4 `ReanchorAtPatchTime`"
            // The anchor becomes patch time. Previous enabled observation stays preserved.
            classified.pending = Some(PendingWorkRule::RecomputeDeadline);
            classified.losses.push(loss(
                LossClass::Conditional,
                "periodic_schedule",
                "reanchor_at_patch_time",
            ));
            classified.provenance = Some(ProvenanceRule::Retime);
        }
        PeriodicMigration::CancelSchedule => {
            // SPEC: docs/specs/reconfiguration_and_topology_patch_spec.md "51.5 `CancelSchedule`"
            // Anchor, boundary, and previous-enabled state are cleared.
            classified.pending = Some(PendingWorkRule::Cancel);
            classified.state = Some(StateCompatibility::Reset);
            classified.episode = Some(EpisodeRule::Terminate);
            classified.provenance = Some(ProvenanceRule::Reset);
            classified
                .losses
                .push(loss(LossClass::Unavoidable, "periodic_enable", "cancel"));
            classified
                .losses
                .push(loss(LossClass::Conditional, "periodic_schedule", "cancel"));
        }
        PeriodicMigration::RejectIfAnchored => {
            classified.pending = Some(PendingWorkRule::Conditional {
                fact: "periodic_anchor",
                when_clear: PendingArm::PreserveDeadline,
                when_set: PendingArm::Reject,
            });
        }
    }
    classified
}

fn classify_standard<D>(source: &NodeKind<D>, target: &NodeKind<D>) -> Classified {
    match (family_of(source), family_of(target)) {
        (Family::Combinational, Family::Combinational) => blank(),
        (Family::Combinational, _) => target_initialized(target),
        (_, Family::Combinational) => source_discarded(source, "stateless_target"),
        (Family::Edge(left), Family::Edge(right)) => edge_rule(left == right),
        (Family::Boolean(left), Family::Boolean(right)) => boolean_standard(left == right),
        (Family::Temporal(left), Family::Temporal(right)) if left == right => {
            temporal_standard(source, target, left)
        }
        _ => unrelated_reject(source, target),
    }
}

fn classify_pair<D>(
    source: &NodeKind<D>,
    target: &NodeKind<D>,
    directive: NodeMigrationDirective<D>,
) -> Classified {
    let source_family = family_of(source);
    let target_family = family_of(target);
    let temporal = temporal_side(source) || temporal_side(target);
    if !directive_fits(directive, source_family, target_family) {
        return failed(ClassifiedError::Incompatible, temporal);
    }
    if let (Family::Temporal(left), Family::Temporal(right)) = (source_family, target_family) {
        if left != right {
            return if matches!(directive, NodeMigrationDirective::Reset) {
                reset_owner(source, target)
            } else {
                failed(ClassifiedError::CrossKind, true)
            };
        }
    }
    if matches!(directive, NodeMigrationDirective::Reset) {
        return reset_owner(source, target);
    }
    if matches!(directive, NodeMigrationDirective::RequirePreserve) {
        let standard = classify_standard(source, target);
        return if preserves(&standard) {
            standard
        } else {
            failed(ClassifiedError::Incompatible, temporal)
        };
    }
    match directive {
        NodeMigrationDirective::Standard => classify_standard(source, target),
        NodeMigrationDirective::TransferStoredLevel => boolean_transfer(matches!(
            (source_family, target_family),
            (Family::Boolean(left), Family::Boolean(right)) if left == right
        )),
        NodeMigrationDirective::PulseDelay(policy) => pulse_policy(policy),
        NodeMigrationDirective::TransportDelay(policy) => transport_policy(policy),
        NodeMigrationDirective::InertialDelay(policy) => inertial_policy(policy),
        NodeMigrationDirective::Periodic(policy) => periodic_policy(policy),
        NodeMigrationDirective::RequirePreserve | NodeMigrationDirective::Reset => blank(),
        NodeMigrationDirective::__Domain(_, never) => match never {},
    }
}

fn metadata_touched<D>(operations: &[PatchOperation<D>], subject: &SubjectRef) -> bool {
    operations.iter().any(|operation| match operation {
        PatchOperation::SetDiagnosticMeta {
            subject: edited, ..
        } => structural_subject(*edited) == *subject,
        _ => false,
    })
}

fn node_hierarchy_touched<D>(operations: &[PatchOperation<D>], key: NodeKey) -> bool {
    operations.iter().any(|operation| {
        matches!(
            operation,
            PatchOperation::SetParent {
                subject: HierarchicalSubjectRef::Node(node),
                parent: Some(_),
            } if *node == key
        )
    })
}

fn replace_directive<D>(
    operations: &[PatchOperation<D>],
    key: NodeKey,
) -> Option<NodeMigrationDirective<D>> {
    operations.iter().find_map(|operation| match operation {
        PatchOperation::ReplaceNode {
            node, migration, ..
        } if *node == key => Some(*migration),
        _ => None,
    })
}

fn reassoc_node_directive<D>(
    operations: &[PatchOperation<D>],
    from: NodeKey,
    to: NodeKey,
) -> Option<NodeMigrationDirective<D>> {
    operations.iter().find_map(|operation| match operation {
        PatchOperation::Reassociate(SubjectReassociation::Node {
            from: source,
            to: destination,
            migration,
        }) if *source == from && *destination == to => Some(*migration),
        _ => None,
    })
}

fn node_links<D>(operations: &[PatchOperation<D>]) -> Vec<(NodeKey, NodeKey)> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::Reassociate(SubjectReassociation::Node { from, to, .. }) => {
                Some((*from, *to))
            }
            _ => None,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn push_node<D: PartialEq>(
    acc: &mut Acc<'_, D>,
    continuity: Continuity,
    metadata_changed: bool,
    hierarchy_changed: bool,
    source: Option<NodeKey>,
    target: Option<NodeKey>,
    state: Option<StateCompatibility>,
    directive: Option<NodeMigrationDirective<D>>,
) {
    acc.subjects.push(SubjectPlan {
        continuity,
        metadata_changed,
        hierarchy_changed,
        source: source.map(SubjectRef::Node),
        target: target.map(SubjectRef::Node),
        state,
        directive,
    });
}

fn classify_nodes<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    acc: &mut Acc<'_, D>,
) {
    let links = node_links(operations);
    let mut claimed = BTreeSet::new();
    for node in base.nodes() {
        if let Some((_, destination)) = links
            .iter()
            .copied()
            .find(|(source, _)| *source == node.key())
        {
            let Some(destination_node) = find_node(target, destination) else {
                continue;
            };
            claimed.insert(destination);
            let directive = reassoc_node_directive(operations, node.key(), destination);
            let classified = classify_pair(
                node.kind(),
                destination_node.kind(),
                directive.unwrap_or(NodeMigrationDirective::Standard),
            );
            acc.commit(
                SubjectRef::Node(destination),
                Some(SubjectRef::Node(node.key())),
                SubjectRef::Node(node.key()),
                &classified,
            );
            push_node(
                acc,
                Continuity::Reassociated,
                metadata_touched(operations, &SubjectRef::Node(node.key()))
                    || metadata_touched(operations, &SubjectRef::Node(destination)),
                node_hierarchy_touched(operations, node.key()),
                Some(node.key()),
                Some(destination),
                classified.state.clone(),
                directive,
            );
        } else if let Some(destination_node) = find_node(target, node.key()) {
            claimed.insert(node.key());
            let directive = replace_directive(operations, node.key());
            let classified = classify_pair(
                node.kind(),
                destination_node.kind(),
                directive.unwrap_or(NodeMigrationDirective::Standard),
            );
            let subject = SubjectRef::Node(node.key());
            acc.commit(subject.clone(), Some(subject.clone()), subject, &classified);
            let same_shape =
                node.kind() == destination_node.kind() && node.ports() == destination_node.ports();
            push_node(
                acc,
                if same_shape {
                    Continuity::Preserved
                } else {
                    Continuity::ReplacedInPlace
                },
                node.meta() != destination_node.meta(),
                node_hierarchy_touched(operations, node.key()),
                Some(node.key()),
                Some(node.key()),
                classified.state.clone(),
                directive,
            );
        } else {
            let classified = source_discarded(node.kind(), "removed");
            let subject = SubjectRef::Node(node.key());
            acc.commit(subject.clone(), None, subject, &classified);
            push_node(
                acc,
                Continuity::Removed,
                false,
                false,
                Some(node.key()),
                None,
                None,
                None,
            );
        }
    }
    for node in target.nodes() {
        if claimed.contains(&node.key()) {
            continue;
        }
        let classified = target_initialized(node.kind());
        acc.commit(
            SubjectRef::Node(node.key()),
            None,
            SubjectRef::Node(node.key()),
            &classified,
        );
        push_node(
            acc,
            Continuity::Added,
            false,
            node_hierarchy_touched(operations, node.key()),
            None,
            Some(node.key()),
            classified.state.clone(),
            None,
        );
    }
}

fn classify_connections<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    acc: &mut Acc<'_, D>,
) {
    for connection in base.connections() {
        let destination = target
            .connections()
            .iter()
            .find(|item| item.key() == connection.key());
        let continuity = match destination {
            Some(destination)
                if connection.from() == destination.from()
                    && connection.to() == destination.to() =>
            {
                Continuity::Preserved
            }
            Some(_) => Continuity::ReplacedInPlace,
            None => Continuity::Removed,
        };
        let subject = SubjectRef::Connection(connection.key());
        acc.subjects.push(SubjectPlan {
            continuity,
            metadata_changed: destination
                .is_some_and(|destination| connection.meta() != destination.meta())
                || metadata_touched(operations, &subject),
            hierarchy_changed: false,
            source: Some(subject),
            target: destination.map(|destination| SubjectRef::Connection(destination.key())),
            state: None,
            directive: None,
        });
    }
    for connection in target.connections() {
        if base
            .connections()
            .iter()
            .any(|item| item.key() == connection.key())
        {
            continue;
        }
        acc.subjects.push(SubjectPlan {
            continuity: Continuity::Added,
            metadata_changed: false,
            hierarchy_changed: false,
            source: None,
            target: Some(SubjectRef::Connection(connection.key())),
            state: None,
            directive: None,
        });
    }
}

fn collect_in_ports<D>(definition: &UncheckedNetwork<D>) -> BTreeSet<AnyInPortKey> {
    definition
        .nodes()
        .iter()
        .flat_map(|node| node.ports().inputs().iter().copied())
        .collect()
}

fn collect_out_ports<D>(definition: &UncheckedNetwork<D>) -> BTreeSet<AnyOutPortKey> {
    definition
        .nodes()
        .iter()
        .flat_map(|node| node.ports().outputs().iter().copied())
        .collect()
}

fn classify_in_ports<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    acc: &mut Acc<'_, D>,
) {
    let base_ports = collect_in_ports(base);
    let target_ports = collect_in_ports(target);
    let links: Vec<(AnyInPortKey, AnyInPortKey)> = operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::Reassociate(SubjectReassociation::InPort { from, to }) => {
                Some((*from, *to))
            }
            _ => None,
        })
        .collect();
    let mut claimed = BTreeSet::new();
    for port in &base_ports {
        if let Some((_, destination)) = links.iter().copied().find(|(source, _)| source == port) {
            if target_ports.contains(&destination) {
                claimed.insert(destination);
                acc.subjects.push(port_plan(
                    Continuity::Reassociated,
                    metadata_touched(operations, &SubjectRef::InPort(*port))
                        || metadata_touched(operations, &SubjectRef::InPort(destination)),
                    Some(SubjectRef::InPort(*port)),
                    Some(SubjectRef::InPort(destination)),
                ));
                continue;
            }
        }
        if target_ports.contains(port) {
            claimed.insert(*port);
            acc.subjects.push(port_plan(
                Continuity::Preserved,
                metadata_touched(operations, &SubjectRef::InPort(*port)),
                Some(SubjectRef::InPort(*port)),
                Some(SubjectRef::InPort(*port)),
            ));
        } else {
            acc.subjects.push(port_plan(
                Continuity::Removed,
                false,
                Some(SubjectRef::InPort(*port)),
                None,
            ));
        }
    }
    for port in &target_ports {
        if claimed.contains(port) || base_ports.contains(port) {
            continue;
        }
        acc.subjects.push(port_plan(
            Continuity::Added,
            false,
            None,
            Some(SubjectRef::InPort(*port)),
        ));
    }
}

fn classify_out_ports<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    acc: &mut Acc<'_, D>,
) {
    let base_ports = collect_out_ports(base);
    let target_ports = collect_out_ports(target);
    let links: Vec<(AnyOutPortKey, AnyOutPortKey)> = operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::Reassociate(SubjectReassociation::OutPort { from, to }) => {
                Some((*from, *to))
            }
            _ => None,
        })
        .collect();
    let mut claimed = BTreeSet::new();
    for port in &base_ports {
        if let Some((_, destination)) = links.iter().copied().find(|(source, _)| source == port) {
            if target_ports.contains(&destination) {
                claimed.insert(destination);
                acc.subjects.push(port_plan(
                    Continuity::Reassociated,
                    metadata_touched(operations, &SubjectRef::OutPort(*port))
                        || metadata_touched(operations, &SubjectRef::OutPort(destination)),
                    Some(SubjectRef::OutPort(*port)),
                    Some(SubjectRef::OutPort(destination)),
                ));
                continue;
            }
        }
        if target_ports.contains(port) {
            claimed.insert(*port);
            acc.subjects.push(port_plan(
                Continuity::Preserved,
                metadata_touched(operations, &SubjectRef::OutPort(*port)),
                Some(SubjectRef::OutPort(*port)),
                Some(SubjectRef::OutPort(*port)),
            ));
        } else {
            acc.subjects.push(port_plan(
                Continuity::Removed,
                false,
                Some(SubjectRef::OutPort(*port)),
                None,
            ));
        }
    }
    for port in &target_ports {
        if claimed.contains(port) || base_ports.contains(port) {
            continue;
        }
        acc.subjects.push(port_plan(
            Continuity::Added,
            false,
            None,
            Some(SubjectRef::OutPort(*port)),
        ));
    }
}

fn port_plan<D>(
    continuity: Continuity,
    metadata_changed: bool,
    source: Option<SubjectRef>,
    target: Option<SubjectRef>,
) -> SubjectPlan<D> {
    SubjectPlan {
        continuity,
        metadata_changed,
        hierarchy_changed: false,
        source,
        target,
        state: None,
        directive: None,
    }
}

fn classify_inputs<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    acc: &mut Acc<'_, D>,
) -> Vec<ExternalInputPlan> {
    let links: Vec<(AnyExternalInputKey, AnyExternalInputKey)> = operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::Reassociate(SubjectReassociation::ExternalInput { from, to }) => {
                Some((*from, *to))
            }
            _ => None,
        })
        .collect();
    let mut plans = Vec::new();
    let mut claimed = BTreeSet::new();
    for input in base.external_inputs() {
        let key = input.key();
        if let Some((_, destination)) = links.iter().copied().find(|(source, _)| *source == key) {
            if target
                .external_inputs()
                .iter()
                .any(|item| item.key() == destination)
            {
                claimed.insert(destination);
                let level = key.kind() == SignalKind::Level;
                plans.push(ExternalInputPlan {
                    continuity: Continuity::Reassociated,
                    source: Some(key),
                    target: Some(destination),
                    valuation: if level {
                        InputValuationPlan::Inherit
                    } else {
                        InputValuationPlan::Preserve
                    },
                });
                acc.subjects.push(port_plan(
                    Continuity::Reassociated,
                    metadata_touched(operations, &SubjectRef::ExternalInput(key)),
                    Some(SubjectRef::ExternalInput(key)),
                    Some(SubjectRef::ExternalInput(destination)),
                ));
                continue;
            }
        }
        if let Some(destination) = target
            .external_inputs()
            .iter()
            .find(|item| item.key() == key)
        {
            claimed.insert(key);
            plans.push(ExternalInputPlan {
                continuity: Continuity::Preserved,
                source: Some(key),
                target: Some(key),
                valuation: InputValuationPlan::Preserve,
            });
            acc.subjects.push(port_plan(
                Continuity::Preserved,
                input.meta() != destination.meta()
                    || metadata_touched(operations, &SubjectRef::ExternalInput(key)),
                Some(SubjectRef::ExternalInput(key)),
                Some(SubjectRef::ExternalInput(key)),
            ));
        } else {
            if key.kind() == SignalKind::Level {
                acc.loss(
                    LossClass::Conditional,
                    SubjectRef::ExternalInput(key),
                    "level_valuation",
                    "removed",
                );
            }
            plans.push(ExternalInputPlan {
                continuity: Continuity::Removed,
                source: Some(key),
                target: None,
                valuation: InputValuationPlan::Remove,
            });
            acc.subjects.push(port_plan(
                Continuity::Removed,
                false,
                Some(SubjectRef::ExternalInput(key)),
                None,
            ));
        }
    }
    for input in target.external_inputs() {
        if claimed.contains(&input.key())
            || base
                .external_inputs()
                .iter()
                .any(|item| item.key() == input.key())
        {
            continue;
        }
        let level = input.key().kind() == SignalKind::Level;
        plans.push(ExternalInputPlan {
            continuity: Continuity::Added,
            source: None,
            target: Some(input.key()),
            valuation: if level {
                InputValuationPlan::Establish
            } else {
                InputValuationPlan::Preserve
            },
        });
        acc.subjects.push(port_plan(
            Continuity::Added,
            false,
            None,
            Some(SubjectRef::ExternalInput(input.key())),
        ));
    }
    plans.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then(left.target.cmp(&right.target))
    });
    plans
}

fn classify_outputs<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    acc: &mut Acc<'_, D>,
) -> Vec<ExternalOutputPlan> {
    let links: Vec<(AnyExternalOutputKey, AnyExternalOutputKey)> = operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::Reassociate(SubjectReassociation::ExternalOutput { from, to }) => {
                Some((*from, *to))
            }
            _ => None,
        })
        .collect();
    let mut plans = Vec::new();
    let mut claimed = BTreeSet::new();
    for output in base.external_outputs() {
        let key = output.key();
        let level = key.kind() == SignalKind::Level;
        if let Some((_, destination)) = links.iter().copied().find(|(source, _)| *source == key) {
            if target
                .external_outputs()
                .iter()
                .any(|item| item.key() == destination)
            {
                claimed.insert(destination);
                plans.push(ExternalOutputPlan {
                    continuity: Continuity::Reassociated,
                    source: Some(key),
                    target: Some(destination),
                    baseline: if level {
                        OutputBaselinePlan::CarryAsEvidence
                    } else {
                        OutputBaselinePlan::Preserve
                    },
                    level,
                });
                acc.subjects.push(port_plan(
                    Continuity::Reassociated,
                    metadata_touched(operations, &SubjectRef::ExternalOutput(key)),
                    Some(SubjectRef::ExternalOutput(key)),
                    Some(SubjectRef::ExternalOutput(destination)),
                ));
                continue;
            }
        }
        if let Some(destination) = target
            .external_outputs()
            .iter()
            .find(|item| item.key() == key)
        {
            claimed.insert(key);
            plans.push(ExternalOutputPlan {
                continuity: Continuity::Preserved,
                source: Some(key),
                target: Some(key),
                baseline: OutputBaselinePlan::Preserve,
                level,
            });
            acc.subjects.push(port_plan(
                Continuity::Preserved,
                output.meta() != destination.meta()
                    || metadata_touched(operations, &SubjectRef::ExternalOutput(key)),
                Some(SubjectRef::ExternalOutput(key)),
                Some(SubjectRef::ExternalOutput(key)),
            ));
        } else {
            if level {
                acc.loss(
                    LossClass::Conditional,
                    SubjectRef::ExternalOutput(key),
                    "level_baseline",
                    "removed",
                );
            }
            plans.push(ExternalOutputPlan {
                continuity: Continuity::Removed,
                source: Some(key),
                target: None,
                baseline: OutputBaselinePlan::Remove,
                level,
            });
            acc.subjects.push(port_plan(
                Continuity::Removed,
                false,
                Some(SubjectRef::ExternalOutput(key)),
                None,
            ));
        }
    }
    for output in target.external_outputs() {
        if claimed.contains(&output.key())
            || base
                .external_outputs()
                .iter()
                .any(|item| item.key() == output.key())
        {
            continue;
        }
        let level = output.key().kind() == SignalKind::Level;
        plans.push(ExternalOutputPlan {
            continuity: Continuity::Added,
            source: None,
            target: Some(output.key()),
            baseline: if level {
                OutputBaselinePlan::Establish
            } else {
                OutputBaselinePlan::Preserve
            },
            level,
        });
        acc.subjects.push(port_plan(
            Continuity::Added,
            false,
            None,
            Some(SubjectRef::ExternalOutput(output.key())),
        ));
    }
    plans.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then(left.target.cmp(&right.target))
    });
    plans
}

fn find_module<D>(
    definition: &UncheckedNetwork<D>,
    key: ModuleInstanceKey,
) -> Option<&ModuleInstanceDef<D>> {
    definition
        .module_instances()
        .iter()
        .find(|instance| instance.key() == key)
}

fn module_directive<D: PartialEq>(
    operations: &[PatchOperation<D>],
    source: Option<ModuleInstanceKey>,
    target: Option<ModuleInstanceKey>,
) -> ModuleMigrationDirective<D> {
    for operation in operations {
        match operation {
            PatchOperation::ReplaceModuleInstance {
                module, migration, ..
            } if source == Some(*module) => {
                return migration.clone();
            }
            PatchOperation::Reassociate(SubjectReassociation::ModuleInstance {
                from,
                to,
                migration,
            }) if source == Some(*from) && target == Some(*to) => {
                return migration.clone();
            }
            _ => {}
        }
    }
    ModuleMigrationDirective::Standard
}

fn internal_directive<D>(
    directive: &ModuleMigrationDirective<D>,
    key: NodeKey,
) -> NodeMigrationDirective<D> {
    match directive {
        ModuleMigrationDirective::Standard => NodeMigrationDirective::Standard,
        ModuleMigrationDirective::Explicit {
            node_overrides,
            internal_reassociations,
        } => internal_reassociations
            .iter()
            .find(|link| link.from() == key || link.to() == key)
            .map(|link| link.migration())
            .or_else(|| {
                node_overrides
                    .iter()
                    .find(|item| item.node() == key)
                    .map(|item| item.migration())
            })
            .unwrap_or(NodeMigrationDirective::Standard),
        ModuleMigrationDirective::__Domain(_, never) => match *never {},
    }
}

fn category_rank(category: StandardInternalCategory) -> u8 {
    match category {
        StandardInternalCategory::Node => 0,
        StandardInternalCategory::InputPort => 1,
        StandardInternalCategory::OutputPort => 2,
        StandardInternalCategory::Connection => 3,
        StandardInternalCategory::Export => 4,
    }
}

fn in_port_raw(port: AnyInPortKey) -> u128 {
    match port {
        AnyInPortKey::Level(key) => key.as_u128(),
        AnyInPortKey::Pulse(key) => key.as_u128(),
    }
}

fn out_port_raw(port: AnyOutPortKey) -> u128 {
    match port {
        AnyOutPortKey::Level(key) => key.as_u128(),
        AnyOutPortKey::Pulse(key) => key.as_u128(),
    }
}

fn role_subject<D>(
    module: &ModuleDef<D>,
    category: StandardInternalCategory,
    raw: u128,
) -> SubjectRef {
    match category {
        StandardInternalCategory::Node => SubjectRef::Node(NodeKey::from_u128(raw)),
        StandardInternalCategory::Connection | StandardInternalCategory::Export => {
            SubjectRef::Connection(ConnectionKey::from_u128(raw))
        }
        StandardInternalCategory::InputPort => module
            .graph()
            .nodes()
            .iter()
            .flat_map(|node| node.ports().inputs().iter().copied())
            .find(|port| in_port_raw(*port) == raw)
            .map(SubjectRef::InPort)
            .unwrap_or_else(|| SubjectRef::Node(NodeKey::from_u128(raw))),
        StandardInternalCategory::OutputPort => module
            .graph()
            .nodes()
            .iter()
            .flat_map(|node| node.ports().outputs().iter().copied())
            .find(|port| out_port_raw(*port) == raw)
            .map(SubjectRef::OutPort)
            .unwrap_or_else(|| SubjectRef::Node(NodeKey::from_u128(raw))),
    }
}

struct RoleRef<D> {
    pair: (u8, String),
    role: String,
    node: Option<(NodeKey, NodeKind<D>)>,
    subject: SubjectRef,
}

fn qualified_node(path: &[ModuleInstanceKey], key: NodeKey) -> SubjectRef {
    SubjectRef::QualifiedNode(QualifiedNodeRef::new(path.to_vec(), key))
}

fn module_roles<D>(path: &[ModuleInstanceKey], module: &ModuleDef<D>) -> Vec<RoleRef<D>> {
    if let Some(declaration) = module.standard_declaration() {
        return declaration
            .internal_roles()
            .map(|role| {
                let node = if role.category() == StandardInternalCategory::Node {
                    let key = NodeKey::from_u128(role.key());
                    module
                        .graph()
                        .nodes()
                        .iter()
                        .find(|node| node.key() == key)
                        .map(|node| (key, node.kind().clone()))
                } else {
                    None
                };
                let subject = node
                    .as_ref()
                    .map(|(key, _)| qualified_node(path, *key))
                    .unwrap_or_else(|| role_subject(module, role.category(), role.key()));
                RoleRef {
                    pair: (category_rank(role.category()), role.role().to_owned()),
                    role: role.role().to_owned(),
                    node,
                    subject,
                }
            })
            .collect();
    }
    module
        .graph()
        .nodes()
        .iter()
        .map(|node| RoleRef {
            pair: (0, format!("{:032x}", node.key().as_u128())),
            role: format!("{:032x}", node.key().as_u128()),
            node: Some((node.key(), node.kind().clone())),
            subject: qualified_node(path, node.key()),
        })
        .collect()
}

fn parameter_changed<D>(
    source: &StandardModuleDeclaration<D>,
    target: &StandardModuleDeclaration<D>,
    key: &str,
) -> bool {
    let left = source
        .parameters()
        .find(|item| item.key().as_str() == key)
        .map(StandardParameterAssignment::value);
    let right = target
        .parameters()
        .find(|item| item.key().as_str() == key)
        .map(StandardParameterAssignment::value);
    matches!((left, right), (Some(left), Some(right)) if left != right)
}

struct InternalDraft<D> {
    role: String,
    continuity: Continuity,
    source_key: Option<NodeKey>,
    target_key: Option<NodeKey>,
    source_kind: Option<NodeKind<D>>,
    target_kind: Option<NodeKind<D>>,
    source: Option<SubjectRef>,
    target: Option<SubjectRef>,
    classified: Classified,
}

fn modules_pair<D>(source: &ModuleDef<D>, target: &ModuleDef<D>) -> bool {
    match (source.standard_declaration(), target.standard_declaration()) {
        (Some(left), Some(right)) => left.module_ref() == right.module_ref(),
        (None, None) => true,
        _ => false,
    }
}

fn reset_to_applies<D>(source: &ModuleDef<D>, target: &ModuleDef<D>) -> bool {
    let (Some(left), Some(right)) = (source.standard_declaration(), target.standard_declaration())
    else {
        return false;
    };
    left.module_ref() == right.module_ref()
        && left.module_ref().id().as_str() == "mossignal.standard.level_resettable_sample_hold"
        && parameter_changed(left, right, "reset_to")
}

fn paired_internal<D: PartialEq>(
    source: &RoleRef<D>,
    target: &RoleRef<D>,
    directive: &ModuleMigrationDirective<D>,
    reset_to: bool,
) -> InternalDraft<D> {
    let source_key = source.node.as_ref().map(|(key, _)| *key);
    let target_key = target.node.as_ref().map(|(key, _)| *key);
    let node_directive = source_key
        .or(target_key)
        .map(|key| internal_directive(directive, key))
        .unwrap_or(NodeMigrationDirective::Standard);
    let mut classified = match (&source.node, &target.node) {
        (Some((_, left)), Some((_, right))) => classify_pair(left, right, node_directive),
        (Some((_, left)), None) => source_discarded(left, "removed"),
        (None, Some((_, right))) => target_initialized(right),
        (None, None) => blank(),
    };
    // SPEC: docs/specs/contracts/structural-preparation.yaml "module-and-internal-correspondence"
    // reset_to stays a predicate. Preparation does not read the settled reset level.
    if reset_to
        && matches!(
            (
                source.node.as_ref().map(|(_, kind)| family_of(kind)),
                target.node.as_ref().map(|(_, kind)| family_of(kind)),
            ),
            (
                Some(Family::Boolean(BooleanKind::SampleHold)),
                Some(Family::Boolean(BooleanKind::SampleHold))
            )
        )
        && matches!(
            node_directive,
            NodeMigrationDirective::Standard | NodeMigrationDirective::RequirePreserve
        )
    {
        classified.state = Some(StateCompatibility::Conditional {
            fact: "reset_to",
            when_clear: ConditionalArm::Preserve,
            when_set: ConditionalArm::Migrate,
        });
        classified.losses.clear();
    }
    // SPEC: docs/specs/contracts/structural-preparation.yaml "static-migration-classes"
    // Equal families do not preserve a changed node definition.
    let continuity = match (&source.node, &target.node) {
        (Some((_, left)), Some((_, right))) if left == right => Continuity::Preserved,
        (None, None) => Continuity::Preserved,
        _ => Continuity::ReplacedInPlace,
    };
    InternalDraft {
        role: source.role.clone(),
        continuity,
        source_key,
        target_key,
        source_kind: source.node.as_ref().map(|(_, kind)| kind.clone()),
        target_kind: target.node.as_ref().map(|(_, kind)| kind.clone()),
        source: Some(source.subject.clone()),
        target: Some(target.subject.clone()),
        classified,
    }
}

fn removed_internal<D>(role: &RoleRef<D>) -> InternalDraft<D> {
    let classified = role
        .node
        .as_ref()
        .map(|(_, kind)| source_discarded(kind, "removed"))
        .unwrap_or_else(blank);
    InternalDraft {
        role: role.role.clone(),
        continuity: Continuity::Removed,
        source_key: role.node.as_ref().map(|(key, _)| *key),
        target_key: None,
        source_kind: role.node.as_ref().map(|(_, kind)| kind.clone()),
        target_kind: None,
        source: Some(role.subject.clone()),
        target: None,
        classified,
    }
}

fn added_internal<D>(role: &RoleRef<D>) -> InternalDraft<D> {
    let classified = role
        .node
        .as_ref()
        .map(|(_, kind)| target_initialized(kind))
        .unwrap_or_else(blank);
    InternalDraft {
        role: role.role.clone(),
        continuity: Continuity::Added,
        source_key: None,
        target_key: role.node.as_ref().map(|(key, _)| *key),
        source_kind: None,
        target_kind: role.node.as_ref().map(|(_, kind)| kind.clone()),
        source: None,
        target: Some(role.subject.clone()),
        classified,
    }
}

fn role_drafts<D: PartialEq>(
    source_path: &[ModuleInstanceKey],
    source_module: &ModuleDef<D>,
    target_path: &[ModuleInstanceKey],
    target_module: &ModuleDef<D>,
    directive: &ModuleMigrationDirective<D>,
) -> Vec<InternalDraft<D>> {
    let source_roles = module_roles(source_path, source_module);
    let target_roles = module_roles(target_path, target_module);
    let reset_to = reset_to_applies(source_module, target_module);
    let mut drafts = Vec::new();
    if modules_pair(source_module, target_module) {
        let mut claimed = BTreeSet::new();
        for source in &source_roles {
            if let Some(target) = target_roles.iter().find(|role| role.pair == source.pair) {
                claimed.insert(target.pair.clone());
                drafts.push(paired_internal(source, target, directive, reset_to));
            } else {
                drafts.push(removed_internal(source));
            }
        }
        for target in &target_roles {
            if !claimed.contains(&target.pair) {
                drafts.push(added_internal(target));
            }
        }
    } else {
        drafts.extend(source_roles.iter().map(removed_internal));
        drafts.extend(target_roles.iter().map(added_internal));
    }
    drafts
}

fn extend_path(path: &[ModuleInstanceKey], key: ModuleInstanceKey) -> Vec<ModuleInstanceKey> {
    let mut extended = path.to_vec();
    extended.push(key);
    extended
}

fn push_removed_tree<D>(
    drafts: &mut Vec<InternalDraft<D>>,
    path: &[ModuleInstanceKey],
    module: &ModuleDef<D>,
) {
    for role in module_roles(path, module) {
        drafts.push(removed_internal(&role));
    }
    for nested in module.graph().module_instances().to_vec() {
        let nested_path = extend_path(path, nested.key());
        push_removed_tree(drafts, &nested_path, nested.module());
    }
}

fn push_added_tree<D>(
    drafts: &mut Vec<InternalDraft<D>>,
    path: &[ModuleInstanceKey],
    module: &ModuleDef<D>,
) {
    for role in module_roles(path, module) {
        drafts.push(added_internal(&role));
    }
    for nested in module.graph().module_instances().to_vec() {
        let nested_path = extend_path(path, nested.key());
        push_added_tree(drafts, &nested_path, nested.module());
    }
}

fn append_nested<D: PartialEq>(
    drafts: &mut Vec<InternalDraft<D>>,
    source_module: &ModuleDef<D>,
    target_module: &ModuleDef<D>,
    source_path: &[ModuleInstanceKey],
    target_path: &[ModuleInstanceKey],
    directive: &ModuleMigrationDirective<D>,
) {
    let source_instances = source_module.graph().module_instances().to_vec();
    let target_instances = target_module.graph().module_instances().to_vec();
    if !modules_pair(source_module, target_module) {
        for nested in &source_instances {
            push_removed_tree(
                drafts,
                &extend_path(source_path, nested.key()),
                nested.module(),
            );
        }
        for nested in &target_instances {
            push_added_tree(
                drafts,
                &extend_path(target_path, nested.key()),
                nested.module(),
            );
        }
        return;
    }
    let mut claimed = BTreeSet::new();
    for source in &source_instances {
        if let Some(target) = target_instances
            .iter()
            .find(|item| item.key() == source.key())
        {
            claimed.insert(source.key());
            let source_child = extend_path(source_path, source.key());
            let target_child = extend_path(target_path, target.key());
            drafts.extend(role_drafts(
                &source_child,
                source.module(),
                &target_child,
                target.module(),
                directive,
            ));
            append_nested(
                drafts,
                source.module(),
                target.module(),
                &source_child,
                &target_child,
                directive,
            );
        } else {
            push_removed_tree(
                drafts,
                &extend_path(source_path, source.key()),
                source.module(),
            );
        }
    }
    for target in &target_instances {
        if !claimed.contains(&target.key()) {
            push_added_tree(
                drafts,
                &extend_path(target_path, target.key()),
                target.module(),
            );
        }
    }
}

fn apply_internal_links<D: PartialEq>(
    drafts: &mut Vec<InternalDraft<D>>,
    directive: &ModuleMigrationDirective<D>,
) -> Vec<NodeKey> {
    let ModuleMigrationDirective::Explicit {
        internal_reassociations,
        ..
    } = directive
    else {
        return Vec::new();
    };
    let mut consumed = BTreeSet::new();
    let mut merged = Vec::new();
    let mut unmatched = Vec::new();
    for link in internal_reassociations {
        let source_index = drafts.iter().position(|draft| {
            draft.continuity == Continuity::Removed && draft.source_key == Some(link.from())
        });
        let target_index = drafts.iter().position(|draft| {
            draft.continuity == Continuity::Added && draft.target_key == Some(link.to())
        });
        let Some(source_index) = source_index else {
            unmatched.push(link.from());
            continue;
        };
        let Some(target_index) = target_index else {
            unmatched.push(link.from());
            continue;
        };
        if source_index == target_index
            || consumed.contains(&source_index)
            || consumed.contains(&target_index)
        {
            unmatched.push(link.from());
            continue;
        }
        consumed.insert(source_index);
        consumed.insert(target_index);
        let source = &drafts[source_index];
        let target = &drafts[target_index];
        let classified = match (&source.source_kind, &target.target_kind) {
            (Some(left), Some(right)) => classify_pair(left, right, link.migration()),
            _ => blank(),
        };
        merged.push(InternalDraft {
            role: source.role.clone(),
            continuity: Continuity::Reassociated,
            source_key: source.source_key,
            target_key: target.target_key,
            source_kind: source.source_kind.clone(),
            target_kind: target.target_kind.clone(),
            source: source.source.clone(),
            target: target.target.clone(),
            classified,
        });
    }
    let mut remove_at: Vec<usize> = consumed.into_iter().collect();
    remove_at.sort_unstable();
    for index in remove_at.into_iter().rev() {
        drafts.remove(index);
    }
    drafts.extend(merged);
    unmatched
}

fn internal_drafts<D: PartialEq>(
    source: &ModuleInstanceDef<D>,
    target: &ModuleInstanceDef<D>,
    directive: &ModuleMigrationDirective<D>,
) -> (Vec<InternalDraft<D>>, Vec<NodeKey>) {
    let mut drafts = role_drafts(
        &[source.key()],
        source.module(),
        &[target.key()],
        target.module(),
        directive,
    );
    append_nested(
        &mut drafts,
        source.module(),
        target.module(),
        &[source.key()],
        &[target.key()],
        directive,
    );
    let unmatched = apply_internal_links(&mut drafts, directive);
    (drafts, unmatched)
}

fn commit_internal<D: PartialEq>(acc: &mut Acc<'_, D>, draft: &InternalDraft<D>) {
    let (event_subject, predecessor, loss_subject) = match draft.continuity {
        Continuity::Removed => (draft.source.clone(), None, draft.source.clone()),
        Continuity::Added => (draft.target.clone(), None, draft.target.clone()),
        Continuity::Preserved | Continuity::ReplacedInPlace | Continuity::Reassociated => (
            draft.target.clone(),
            draft.source.clone(),
            draft.source.clone().or_else(|| draft.target.clone()),
        ),
    };
    if let (Some(event_subject), Some(loss_subject)) = (event_subject, loss_subject) {
        acc.commit(event_subject, predecessor, loss_subject, &draft.classified);
    }
}

fn module_record<D>(
    continuity: Continuity,
    source: Option<ModuleInstanceKey>,
    target: Option<ModuleInstanceKey>,
    drafts: Vec<InternalDraft<D>>,
    directive: ModuleMigrationDirective<D>,
) -> ModuleContinuity<D> {
    let mut internals = Vec::with_capacity(drafts.len());
    for draft in drafts {
        internals.push(InternalSubjectPlan {
            role: draft.role,
            continuity: draft.continuity,
            source: draft.source,
            target: draft.target,
            state: draft.classified.state,
        });
    }
    internals.sort_by(|left, right| {
        left.role
            .cmp(&right.role)
            .then(left.source.cmp(&right.source))
            .then(left.target.cmp(&right.target))
    });
    ModuleContinuity {
        continuity,
        source,
        target,
        internals,
        directive,
    }
}

fn module_semantics_match<D>(left: &ModuleInstanceDef<D>, right: &ModuleInstanceDef<D>) -> bool {
    left.module().fingerprint() == right.module().fingerprint()
        && left.bindings() == right.bindings()
        && left.parent() == right.parent()
}

fn module_subject_plan<D>(
    continuity: Continuity,
    metadata_changed: bool,
    hierarchy_changed: bool,
    source: Option<ModuleInstanceKey>,
    target: Option<ModuleInstanceKey>,
) -> SubjectPlan<D> {
    SubjectPlan {
        continuity,
        metadata_changed,
        hierarchy_changed,
        source: source.map(SubjectRef::ModuleInstance),
        target: target.map(SubjectRef::ModuleInstance),
        state: None,
        directive: None,
    }
}

fn removed_module_drafts<D>(instance: &ModuleInstanceDef<D>) -> Vec<InternalDraft<D>> {
    let mut drafts = Vec::new();
    push_removed_tree(&mut drafts, &[instance.key()], instance.module());
    drafts
}

fn added_module_drafts<D>(instance: &ModuleInstanceDef<D>) -> Vec<InternalDraft<D>> {
    let mut drafts = Vec::new();
    push_added_tree(&mut drafts, &[instance.key()], instance.module());
    drafts
}

fn classify_modules<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    acc: &mut Acc<'_, D>,
) -> Vec<ModuleContinuity<D>> {
    let links: Vec<(ModuleInstanceKey, ModuleInstanceKey)> = operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::Reassociate(SubjectReassociation::ModuleInstance {
                from, to, ..
            }) => Some((*from, *to)),
            _ => None,
        })
        .collect();
    let mut records = Vec::new();
    let mut claimed = BTreeSet::new();
    for instance in base.module_instances() {
        if let Some((_, destination_key)) = links
            .iter()
            .copied()
            .find(|(source, _)| *source == instance.key())
        {
            let Some(destination) = find_module(target, destination_key) else {
                continue;
            };
            claimed.insert(destination_key);
            let directive =
                module_directive(operations, Some(instance.key()), Some(destination_key));
            let (drafts, unmatched) = internal_drafts(instance, destination, &directive);
            for key in unmatched {
                acc.error(
                    qualified_node(&[instance.key()], key),
                    ClassifiedError::Incompatible,
                );
            }
            for draft in &drafts {
                commit_internal(acc, draft);
            }
            let metadata_changed = instance.meta() != destination.meta()
                || metadata_touched(operations, &SubjectRef::ModuleInstance(instance.key()))
                || metadata_touched(operations, &SubjectRef::ModuleInstance(destination_key));
            let hierarchy_changed = instance.parent() != destination.parent();
            acc.subjects.push(module_subject_plan(
                Continuity::Reassociated,
                metadata_changed,
                hierarchy_changed,
                Some(instance.key()),
                Some(destination_key),
            ));
            records.push(module_record(
                Continuity::Reassociated,
                Some(instance.key()),
                Some(destination_key),
                drafts,
                directive,
            ));
            continue;
        }
        if let Some(destination) = find_module(target, instance.key()) {
            claimed.insert(instance.key());
            let directive =
                module_directive(operations, Some(instance.key()), Some(instance.key()));
            let (drafts, unmatched) = internal_drafts(instance, destination, &directive);
            for key in unmatched {
                acc.error(
                    qualified_node(&[instance.key()], key),
                    ClassifiedError::Incompatible,
                );
            }
            for draft in &drafts {
                commit_internal(acc, draft);
            }
            let continuity = if module_semantics_match(instance, destination) {
                Continuity::Preserved
            } else {
                Continuity::ReplacedInPlace
            };
            let metadata_changed = instance.meta() != destination.meta()
                || metadata_touched(operations, &SubjectRef::ModuleInstance(instance.key()));
            let hierarchy_changed = instance.parent() != destination.parent();
            acc.subjects.push(module_subject_plan(
                continuity,
                metadata_changed,
                hierarchy_changed,
                Some(instance.key()),
                Some(instance.key()),
            ));
            records.push(module_record(
                continuity,
                Some(instance.key()),
                Some(instance.key()),
                drafts,
                directive,
            ));
        } else {
            let drafts = removed_module_drafts(instance);
            for draft in &drafts {
                commit_internal(acc, draft);
            }
            acc.subjects.push(module_subject_plan(
                Continuity::Removed,
                false,
                false,
                Some(instance.key()),
                None,
            ));
            records.push(module_record(
                Continuity::Removed,
                Some(instance.key()),
                None,
                drafts,
                ModuleMigrationDirective::Standard,
            ));
        }
    }
    for instance in target.module_instances() {
        if claimed.contains(&instance.key()) || find_module(base, instance.key()).is_some() {
            continue;
        }
        let drafts = added_module_drafts(instance);
        for draft in &drafts {
            commit_internal(acc, draft);
        }
        acc.subjects.push(module_subject_plan(
            Continuity::Added,
            false,
            false,
            None,
            Some(instance.key()),
        ));
        records.push(module_record(
            Continuity::Added,
            None,
            Some(instance.key()),
            drafts,
            ModuleMigrationDirective::Standard,
        ));
    }
    records.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then(left.target.cmp(&right.target))
    });
    records
}

fn ensure_region(parent: &mut BTreeMap<SubjectRef, SubjectRef>, subject: SubjectRef) {
    parent.entry(subject.clone()).or_insert(subject);
}

fn find_region(parent: &mut BTreeMap<SubjectRef, SubjectRef>, subject: &SubjectRef) -> SubjectRef {
    let mut current = subject.clone();
    let mut seen = Vec::new();
    loop {
        let next = parent
            .get(&current)
            .cloned()
            .unwrap_or_else(|| current.clone());
        if next == current {
            break;
        }
        seen.push(current);
        current = next;
    }
    for item in seen {
        parent.insert(item, current.clone());
    }
    current
}

fn unite_region(
    parent: &mut BTreeMap<SubjectRef, SubjectRef>,
    left: SubjectRef,
    right: SubjectRef,
) {
    ensure_region(parent, left.clone());
    ensure_region(parent, right.clone());
    let left_root = find_region(parent, &left);
    let right_root = find_region(parent, &right);
    if left_root == right_root {
        return;
    }
    if left_root < right_root {
        parent.insert(right_root, left_root);
    } else {
        parent.insert(left_root, right_root);
    }
}

fn endpoint_subject<D>(
    definition: &UncheckedNetwork<D>,
    endpoint: ConnectionEndpoint,
) -> Option<SubjectRef> {
    match endpoint {
        ConnectionEndpoint::ExternalInput(key) => Some(SubjectRef::ExternalInput(key)),
        ConnectionEndpoint::ExternalOutput(key) => Some(SubjectRef::ExternalOutput(key)),
        ConnectionEndpoint::NodeInput(key) => definition
            .nodes()
            .iter()
            .find(|node| node.ports().inputs().contains(&key))
            .map(|node| SubjectRef::Node(node.key())),
        ConnectionEndpoint::NodeOutput(key) => definition
            .nodes()
            .iter()
            .find(|node| node.ports().outputs().contains(&key))
            .map(|node| SubjectRef::Node(node.key())),
        ConnectionEndpoint::ModuleOutput { instance, .. } => {
            Some(SubjectRef::ModuleInstance(instance))
        }
        ConnectionEndpoint::ModuleInput(_) => None,
    }
}

fn signal_subject<D>(
    definition: &UncheckedNetwork<D>,
    source: AnySignalSourceKey,
) -> Option<SubjectRef> {
    match source {
        AnySignalSourceKey::Level(SignalSourceKey::ExternalInput(key)) => {
            Some(SubjectRef::ExternalInput(key.into()))
        }
        AnySignalSourceKey::Pulse(SignalSourceKey::ExternalInput(key)) => {
            Some(SubjectRef::ExternalInput(key.into()))
        }
        AnySignalSourceKey::Level(SignalSourceKey::NodeOutput(key)) => definition
            .nodes()
            .iter()
            .find(|node| node.ports().outputs().contains(&key.into()))
            .map(|node| SubjectRef::Node(node.key())),
        AnySignalSourceKey::Pulse(SignalSourceKey::NodeOutput(key)) => definition
            .nodes()
            .iter()
            .find(|node| node.ports().outputs().contains(&key.into()))
            .map(|node| SubjectRef::Node(node.key())),
        AnySignalSourceKey::Level(SignalSourceKey::ModuleOutput { instance, .. })
        | AnySignalSourceKey::Pulse(SignalSourceKey::ModuleOutput { instance, .. }) => {
            Some(SubjectRef::ModuleInstance(instance))
        }
    }
}

fn component_roots<D>(definition: &UncheckedNetwork<D>) -> BTreeMap<SubjectRef, SubjectRef> {
    let mut parent = BTreeMap::new();
    for node in definition.nodes() {
        ensure_region(&mut parent, SubjectRef::Node(node.key()));
    }
    for input in definition.external_inputs() {
        ensure_region(&mut parent, SubjectRef::ExternalInput(input.key()));
    }
    for output in definition.external_outputs() {
        ensure_region(&mut parent, SubjectRef::ExternalOutput(output.key()));
    }
    for instance in definition.module_instances() {
        ensure_region(&mut parent, SubjectRef::ModuleInstance(instance.key()));
    }
    for connection in definition.connections() {
        let from = endpoint_subject(definition, connection.from());
        let to = endpoint_subject(definition, connection.to());
        if let (Some(from), Some(to)) = (from, to) {
            unite_region(&mut parent, from, to);
        }
    }
    for output in definition.external_outputs() {
        if let Some(source) = signal_subject(definition, output.source()) {
            unite_region(
                &mut parent,
                SubjectRef::ExternalOutput(output.key()),
                source,
            );
        }
    }
    for instance in definition.module_instances() {
        for binding in instance.bindings().bindings() {
            if let Some(source) = endpoint_subject(definition, binding.source()) {
                unite_region(
                    &mut parent,
                    SubjectRef::ModuleInstance(instance.key()),
                    source,
                );
            }
        }
    }
    let subjects: Vec<SubjectRef> = parent.keys().cloned().collect();
    let mut roots = BTreeMap::new();
    for subject in subjects {
        let root = find_region(&mut parent, &subject);
        roots.insert(subject, root);
    }
    roots
}

fn mapped_subject<D>(operations: &[PatchOperation<D>], subject: &SubjectRef) -> Option<SubjectRef> {
    operations.iter().find_map(|operation| match operation {
        PatchOperation::Reassociate(mapping) => {
            let (from, to) = reassoc_ends(mapping);
            (from == *subject).then_some(to)
        }
        _ => None,
    })
}

fn region_changes<D>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
) -> Vec<RegionChange> {
    let base_roots = component_roots(base);
    let target_roots = component_roots(target);
    let mut pairs = BTreeSet::new();
    for (subject, base_root) in &base_roots {
        let image = mapped_subject(operations, subject).unwrap_or_else(|| subject.clone());
        if let Some(target_root) = target_roots.get(&image) {
            pairs.insert((base_root.clone(), target_root.clone()));
        }
    }
    let mut by_target: BTreeMap<SubjectRef, BTreeSet<SubjectRef>> = BTreeMap::new();
    let mut by_base: BTreeMap<SubjectRef, BTreeSet<SubjectRef>> = BTreeMap::new();
    for (base_root, target_root) in pairs {
        by_target
            .entry(target_root.clone())
            .or_default()
            .insert(base_root.clone());
        by_base.entry(base_root).or_default().insert(target_root);
    }
    let mut changes = Vec::new();
    for (into, from) in by_target {
        if from.len() > 1 {
            changes.push(RegionChange {
                kind: RegionChangeKind::Merge,
                from: from.into_iter().collect(),
                into: vec![into],
            });
        }
    }
    for (from, into) in by_base {
        if into.len() > 1 {
            changes.push(RegionChange {
                kind: RegionChangeKind::Split,
                from: vec![from],
                into: into.into_iter().collect(),
            });
        }
    }
    changes.sort_by(|left, right| {
        region_rank(left.kind)
            .cmp(&region_rank(right.kind))
            .then(left.from.cmp(&right.from))
            .then(left.into.cmp(&right.into))
    });
    changes
}

fn region_rank(kind: RegionChangeKind) -> u8 {
    match kind {
        RegionChangeKind::Merge => 0,
        RegionChangeKind::Split => 1,
    }
}

fn is_endpoint(subject: &SubjectRef) -> bool {
    matches!(
        subject,
        SubjectRef::InPort(_)
            | SubjectRef::OutPort(_)
            | SubjectRef::ExternalInput(_)
            | SubjectRef::ExternalOutput(_)
    )
}

fn rebinding_notices<D>(subjects: &[SubjectPlan<D>]) -> Vec<EndpointRebinding> {
    let mut notices = Vec::new();
    for plan in subjects {
        if !plan
            .source
            .as_ref()
            .or(plan.target.as_ref())
            .is_some_and(is_endpoint)
        {
            continue;
        }
        let change = match plan.continuity {
            Continuity::Added => Some(EndpointChange::Added),
            Continuity::Removed => Some(EndpointChange::Removed),
            Continuity::Reassociated => Some(EndpointChange::Reassociated),
            Continuity::Preserved | Continuity::ReplacedInPlace => None,
        };
        let Some(change) = change else {
            continue;
        };
        notices.push(EndpointRebinding {
            change,
            source: plan.source.clone(),
            target: plan.target.clone(),
        });
    }
    notices.sort_by(|left, right| {
        endpoint_rank(left.change)
            .cmp(&endpoint_rank(right.change))
            .then(left.source.cmp(&right.source))
            .then(left.target.cmp(&right.target))
    });
    notices
}

fn endpoint_rank(change: EndpointChange) -> u8 {
    match change {
        EndpointChange::Added => 0,
        EndpointChange::Removed => 1,
        EndpointChange::Reassociated => 2,
    }
}

fn subject_rank(subject: &SubjectRef) -> u8 {
    match subject {
        SubjectRef::Node(_) | SubjectRef::QualifiedNode(_) => 0,
        SubjectRef::InPort(_) => 1,
        SubjectRef::OutPort(_) => 2,
        SubjectRef::Connection(_) => 3,
        SubjectRef::ExternalInput(_) => 4,
        SubjectRef::ExternalOutput(_) => 5,
        SubjectRef::ModuleInstance(_) => 6,
        _ => 7,
    }
}

fn plan_rank<D>(plan: &SubjectPlan<D>) -> u8 {
    plan.source
        .as_ref()
        .or(plan.target.as_ref())
        .map(subject_rank)
        .unwrap_or(7)
}

fn loss_rank(class: LossClass) -> u8 {
    match class {
        LossClass::Unavoidable => 0,
        LossClass::Conditional => 1,
    }
}

fn episode_rank(rule: EpisodeRule) -> u8 {
    match rule {
        EpisodeRule::Preserve => 0,
        EpisodeRule::Transform => 1,
        EpisodeRule::Resolve => 2,
        EpisodeRule::Terminate => 3,
        EpisodeRule::Reject => 4,
    }
}

fn provenance_rank(rule: ProvenanceRule) -> u8 {
    match rule {
        ProvenanceRule::Checkpoint => 0,
        ProvenanceRule::Reset => 1,
        ProvenanceRule::Retime => 2,
        ProvenanceRule::Loss => 3,
    }
}

fn audit_events<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    events: &[EventRule],
    diagnostics: &mut DiagnosticSet<D>,
) {
    let mut seen = BTreeSet::new();
    for rule in events {
        if !seen.insert(rule.subject.clone()) {
            push_problem(
                diagnostics,
                rule.subject.clone(),
                Vec::new(),
                ProblemEvidence::ReconfigurationAmbiguousEventMigration {
                    marker: PhantomData,
                },
            );
        }
    }
    let mut temporal = BTreeSet::new();
    for node in base.nodes().iter().chain(target.nodes()) {
        if node_schema(node.kind()).temporal_family().is_some() {
            temporal.insert(node.key());
        }
    }
    for key in temporal {
        let subject = SubjectRef::Node(key);
        let covered = events
            .iter()
            .any(|rule| rule.subject == subject || rule.predecessor.as_ref() == Some(&subject));
        if !covered {
            push_problem(
                diagnostics,
                subject,
                Vec::new(),
                ProblemEvidence::ReconfigurationIncompleteTemporalMigrationPolicy {
                    marker: PhantomData,
                },
            );
        }
    }
}

fn classify<D: PartialEq>(
    base: &UncheckedNetwork<D>,
    target: &UncheckedNetwork<D>,
    operations: &[PatchOperation<D>],
    base_schema: InputSchemaFingerprint,
    target_schema: InputSchemaFingerprint,
    diagnostics: &mut DiagnosticSet<D>,
) -> StaticMigrationPlan<D> {
    let mut subjects = Vec::new();
    let mut events = Vec::new();
    let mut episodes = Vec::new();
    let mut provenance = Vec::new();
    let mut losses = Vec::new();
    let (external_inputs, external_outputs, modules) = {
        let mut acc = Acc {
            diagnostics,
            subjects: &mut subjects,
            events: &mut events,
            losses: &mut losses,
            episodes: &mut episodes,
            provenance: &mut provenance,
        };
        classify_nodes(base, target, operations, &mut acc);
        classify_connections(base, target, operations, &mut acc);
        classify_in_ports(base, target, operations, &mut acc);
        classify_out_ports(base, target, operations, &mut acc);
        let external_inputs = classify_inputs(base, target, operations, &mut acc);
        let external_outputs = classify_outputs(base, target, operations, &mut acc);
        let modules = classify_modules(base, target, operations, &mut acc);
        (external_inputs, external_outputs, modules)
    };
    audit_events(base, target, &events, diagnostics);
    subjects.sort_by(|left, right| {
        plan_rank(left)
            .cmp(&plan_rank(right))
            .then(left.source.cmp(&right.source))
            .then(left.target.cmp(&right.target))
    });
    events.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then(left.predecessor.cmp(&right.predecessor))
    });
    episodes.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then(episode_rank(left.rule).cmp(&episode_rank(right.rule)))
    });
    provenance.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then(provenance_rank(left.rule).cmp(&provenance_rank(right.rule)))
    });
    losses.sort_by(|left, right| {
        loss_rank(left.class)
            .cmp(&loss_rank(right.class))
            .then(left.subject.cmp(&right.subject))
            .then(left.fact.cmp(right.fact))
            .then(left.rule.cmp(right.rule))
    });
    let mut invalidated = vec![
        ArtifactInvalidation::ResolvedHandles,
        ArtifactInvalidation::CompiledInspectionPlans,
        ArtifactInvalidation::OldRevisionPreparedPatches,
    ];
    if base_schema != target_schema {
        invalidated.push(ArtifactInvalidation::SchemaBoundBindingProjectors);
        invalidated.push(ArtifactInvalidation::OldSchemaInputArtifacts);
    }
    let region_changes = region_changes(base, target, operations);
    let rebinding = rebinding_notices(&subjects);
    StaticMigrationPlan {
        subjects,
        event_rules: events,
        external_inputs,
        external_outputs,
        modules,
        episodes,
        provenance,
        potential_losses: losses,
        invalidated,
        region_changes,
        rebinding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::DiagnosticMeta;
    use crate::signal::LogicLevel;
    use crate::{NetworkBuilder, RuntimePolicy, TimeDomainId};

    fn policy() -> RuntimePolicy {
        RuntimePolicy::builder()
            .max_internal_reactions(100)
            .max_evaluated_operations(1_000)
            .max_pending_events(100)
            .max_events_created_per_transaction(100)
            .max_required_provenance_growth(10_000)
            .build()
            .unwrap_or_else(|failure| panic!("policy must build: {failure}"))
    }

    fn compiled() -> CompiledNetwork<()> {
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
            .finish()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("constant network must validate: {failure:?}"))
            .compile()
            .require_artifact()
            .unwrap_or_else(|failure| panic!("constant network must compile: {failure:?}"))
    }

    #[test]
    fn stale_revision_is_rejected_without_mutating_the_machine() {
        let compiled = compiled();
        let machine = compiled.spawn(policy());
        let patch = NetworkPatchBuilder::bound(
            compiled.network_key(),
            compiled.fingerprint(),
            compiled.time_domain_id(),
            NetworkRevision::from_value(4),
        )
        .finish();
        let report = machine.prepare_patch(patch);
        assert!(report.artifact().is_none());
        let codes: Vec<_> = report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.problem().code().as_str())
            .collect();
        assert_eq!(codes, ["reconfiguration.base_revision_mismatch"]);
        assert_eq!(machine.revision().value(), 0);
    }

    #[test]
    fn effective_metadata_patch_proposes_the_next_revision() {
        let compiled = compiled();
        let machine = compiled.spawn(policy());
        let meta = DiagnosticMeta {
            name: Some("renamed".to_owned()),
            ..DiagnosticMeta::default()
        };
        let patch = machine
            .patch()
            .set_diagnostic_meta(StructuralSubjectRef::Network(compiled.network_key()), meta)
            .unwrap_or_else(|failure| panic!("metadata edit must build: {failure:?}"))
            .finish();
        let prepared = machine
            .prepare_patch(patch)
            .require_artifact()
            .unwrap_or_else(|failure| panic!("metadata patch must prepare: {failure:?}"));
        assert_eq!(prepared.base_revision().value(), 0);
        assert_eq!(prepared.proposed_revision().value(), 1);
        assert_eq!(machine.revision().value(), 0);
        assert!(
            prepared
                .static_plan()
                .invalidated()
                .contains(&ArtifactInvalidation::ResolvedHandles)
        );
        assert!(
            prepared
                .static_plan()
                .invalidated()
                .contains(&ArtifactInvalidation::CompiledInspectionPlans)
        );
        assert!(
            prepared
                .static_plan()
                .invalidated()
                .contains(&ArtifactInvalidation::OldRevisionPreparedPatches)
        );
        assert!(
            !prepared
                .static_plan()
                .invalidated()
                .contains(&ArtifactInvalidation::SchemaBoundBindingProjectors)
        );
    }
}
