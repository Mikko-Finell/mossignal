//! Scoped access to the existing committed diagnostic records.

use crate::diagnostics::{DiagnosticOccurrence, SubjectRef};
use crate::key::{ModuleInstanceKey, NodeKey};
use crate::{
    ActiveDiagnosticEpisode, DiagnosticEpisodeChange, ForecastState, GraphElement,
    GraphQueryFailure, GraphSubjectRef, InspectionFailure, Machine, NodeSubject,
    QualifiedModuleRef, TransactionResult,
};

/// A primitive node or a complete module hierarchy used to select diagnostic records.
/// Selection preserves the original primitive owners and record order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticScope {
    Node(NodeSubject),
    Module(QualifiedModuleRef),
}

impl DiagnosticScope {
    /// Selects a node in the containing network.
    #[must_use]
    pub const fn node(node: NodeKey) -> Self {
        Self::Node(NodeSubject::Node(node))
    }

    /// Selects a top-level module and all of its nested primitive owners.
    #[must_use]
    pub fn module(module: ModuleInstanceKey) -> Self {
        Self::Module(QualifiedModuleRef::new(vec![module]))
    }

    fn contains(&self, primary: &SubjectRef) -> bool {
        match (self, primary) {
            (Self::Node(NodeSubject::Node(expected)), SubjectRef::Node(actual)) => {
                expected == actual
            }
            (Self::Node(NodeSubject::Qualified(expected)), SubjectRef::QualifiedNode(actual)) => {
                expected == actual
            }
            (Self::Module(module), SubjectRef::QualifiedNode(node)) => {
                node.instances().starts_with(module.instances())
            }
            _ => false,
        }
    }

    fn graph_subject(&self) -> GraphSubjectRef {
        match self {
            Self::Node(node) => GraphSubjectRef::node(node),
            Self::Module(module) => GraphSubjectRef::qualified(
                module.instances()[..module.instances().len() - 1].to_vec(),
                GraphElement::Module(module.instance()),
            ),
        }
    }
}

impl<D> TransactionResult<D> {
    /// Selects occurrences from this owned result, in their original reaction order.
    /// Absent owners yield an empty selection. No live topology is consulted, so
    /// records remain accessible after their owner is removed from the machine.
    #[must_use]
    pub fn occurrences_for(&self, scope: &DiagnosticScope) -> Vec<&DiagnosticOccurrence<D>> {
        // SPEC: docs/specs/contracts/runtime-diagnostic-occurrences.yaml "module-transparent-occurrences"
        // Filtering never substitutes a module warning for a primitive record.
        self.occurrences()
            .iter()
            .filter(|occurrence| scope.contains(occurrence.problem().primary()))
            .collect()
    }

    /// Selects lifecycle changes whose before or after owner falls within the scope.
    /// This includes resolution and removal termination without requiring the owner
    /// to exist in the current machine. Causes resolve through this result's view.
    #[must_use]
    pub fn diagnostic_episode_changes_for(
        &self,
        scope: &DiagnosticScope,
    ) -> Vec<&DiagnosticEpisodeChange<D>> {
        self.diagnostic_episode_changes()
            .iter()
            .filter(|change| {
                change
                    .before()
                    .is_some_and(|problem| scope.contains(problem.primary()))
                    || change
                        .after()
                        .is_some_and(|problem| scope.contains(problem.primary()))
            })
            .collect()
    }
}

impl<D> Machine<D> {
    /// Returns owned active records for an existing node or module hierarchy.
    /// Reads require initialized state and preserve the records' primitive owners,
    /// original condition times, evidence, and independently retained provenance.
    pub fn active_diagnostic_episodes_for(
        &self,
        scope: &DiagnosticScope,
    ) -> Result<Vec<ActiveDiagnosticEpisode<D>>, InspectionFailure> {
        if !self.is_initialized() {
            return Err(InspectionFailure::NotInitialized);
        }
        let exists = match scope {
            DiagnosticScope::Node(node) => self.compiled.node_definition(node).is_some(),
            DiagnosticScope::Module(module) => self.compiled.module(module).is_some(),
        };
        if !exists {
            return Err(GraphQueryFailure::UnknownSubject(scope.graph_subject()).into());
        }
        Ok(self
            .store
            .active_episodes
            .values()
            .filter(|episode| scope.contains(episode.current().primary()))
            .cloned()
            .collect())
    }
}

impl<D> ForecastState<D> {
    /// Selects the same owned active records from the unpublished forecast state.
    pub fn active_diagnostic_episodes_for(
        &self,
        scope: &DiagnosticScope,
    ) -> Result<Vec<ActiveDiagnosticEpisode<D>>, InspectionFailure> {
        self.machine.active_diagnostic_episodes_for(scope)
    }
}
