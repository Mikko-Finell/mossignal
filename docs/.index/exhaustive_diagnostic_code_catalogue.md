## docs/specs/exhaustive_diagnostic_code_catalogue.md
- ``mossignal` Exhaustive Diagnostic Code Catalogue` [1-76]
  Preview: **Status:** Design specification, version 1 **Diagnostic schema status:** Experimental; not yet compatibility-frozen **Defines:** The unified problem-code namespace; diagnostic and failure structure; severity, responsibility, delivery, evidence, subjects, suggestions, deterministic collection, persistent diagnostic episodes, internal-defect reporting, persistence rules, verification obligations, and the exhaustive initial code catalogue **Does not define:** Rendered English wording, localization, logging backends, editor presentation, host telemetry, post-freeze deprecation periods, or recovery from arbitrary process corruption This specification defines the complete structured problem-reporting system for `mossignal`.
  Symbols: `mossignal`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Exhaustive Diagnostic Code Catalogue > 1. Purpose` [10-29]
  Preview: This specification defines the complete structured problem-reporting system for `mossignal`.
  Symbols: `mossignal`

- ``mossignal` Exhaustive Diagnostic Code Catalogue > 2. Normative language` [30-47]
  Preview: The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.
  Symbols: `mossignal`
  Normative: MUST NOT 1, MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- ``mossignal` Exhaustive Diagnostic Code Catalogue > 3. Relationship to the other specifications` [48-76]
  Preview: This specification is authoritative for: - diagnostic code identity and namespace; - severity and responsibility assignment; - allowed delivery forms; - condition identity and deduplication; - structured evidence requirements; - persistent episode identity and lifecycle; - canonical diagnostic ordering; - diagnostic-schema compatibility.

- `Part I — Unified problem model` [77-203]
  Preview: Every externally observable problem condition and every named internal verification failure belongs to this catalogue.
  Symbols: `Display`, `RuntimeFailure`, `InternalDefect`, `validation.current_reaction_cycle`, `DiagnosticSchemaVersion`
  Normative: MUST NOT 3, MUST 4, MAY 2

- `Part I — Unified problem model > 3. One catalogue, distinct delivery forms` [79-117]
  Preview: Every externally observable problem condition and every named internal verification failure belongs to this catalogue.
  Symbols: `Display`
  Normative: MUST NOT 1, MUST 1

- `Part I — Unified problem model > 4. Internal defects` [118-136]
  Preview: Internal invariant violations use the same code, subject, evidence, ordering, persistence-safe rendering, and regression infrastructure as ordinary diagnostics.
  Symbols: `RuntimeFailure`, `InternalDefect`
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part I — Unified problem model > 5. Codes name conditions` [137-181]
  Preview: Code syntax is: Each segment MUST match: Examples: Codes MUST NOT be shaped as generic categories such as: A code does not identify the API method that detected the condition.
  Symbols: `validation.current_reaction_cycle`
  Normative: MUST NOT 1, MUST 1

- `Part I — Unified problem model > 6. Experimental and stabilized schemas` [182-203]
  Preview: This specification defines an **experimental** diagnostic schema.
  Symbols: `DiagnosticSchemaVersion`
  Normative: MUST 1, MAY 1

- `Part II — Common structured representation` [204-872]
  Preview: The common conceptual representation is: Exact private representation may differ.
  Symbols: `Error`, `severity`, `responsibility`, `primary`, `related`, `evidence`, `ProblemEvidence<D>`, `Fatal`, `Bug`, `mossignal`, `Warning`, `ReportFinding`, `OperationFailure`, `TimeEvidence`, `MigrationEvidence`
  Normative: MUST NOT 5, MUST 5, SHOULD 2, MAY 5

- `Part II — Common structured representation > 7. Problem record` [206-237]
  Preview: The common conceptual representation is: Exact private representation may differ.
  Symbols: `severity`, `responsibility`, `primary`, `related`, `evidence`, `ProblemEvidence<D>`
  Normative: MUST NOT 2, MUST 1, MAY 1

- `Part II — Common structured representation > 8. Severity` [238-265]
  Preview: The public severities are: A semantically valid fact worth surfacing, usually a compatibility, normalization, or resynchronization notice.
  Symbols: `Fatal`, `Bug`

- `Part II — Common structured representation > 8. Severity > 8.1 `Info`` [250-253]
  Preview: A semantically valid fact worth surfacing, usually a compatibility, normalization, or resynchronization notice.

- `Part II — Common structured representation > 8. Severity > 8.2 `Warning`` [254-257]
  Preview: The operation or artifact remains valid, but the condition is suspicious, degenerate, lossy, deprecated, or operationally concerning.

- `Part II — Common structured representation > 8. Severity > 8.3 `Error`` [258-265]
  Preview: The requested artifact or operation cannot be accepted, or an internal invariant has failed.
  Symbols: `Fatal`, `Bug`

- `Part II — Common structured representation > 9. Responsibility` [266-321]
  Preview: Every code has one fixed responsibility classification: No party necessarily acted incorrectly.
  Symbols: `mossignal`

- `Part II — Common structured representation > 9. Responsibility > 9.1 `Advisory`` [284-287]
  Preview: No party necessarily acted incorrectly.

- `Part II — Common structured representation > 9. Responsibility > 9.2 `CallerInput`` [288-293]
  Preview: The supplied definition, value, query, patch, or operation violates a required rule.

- `Part II — Common structured representation > 9. Responsibility > 9.3 `SemanticRejection`` [294-297]
  Preview: The request is structurally meaningful, but explicit node or policy semantics reject it.

- `Part II — Common structured representation > 9. Responsibility > 9.4 `Compatibility`` [298-301]
  Preview: Two otherwise meaningful artifacts, versions, identities, revisions, schemas, or policies cannot be used together.

- `Part II — Common structured representation > 9. Responsibility > 9.5 `ResourceLimit`` [302-305]
  Preview: An explicit runtime, decode, or verification budget was exceeded.

- `Part II — Common structured representation > 9. Responsibility > 9.6 `CorruptData`` [306-309]
  Preview: Persisted or retained data contradicts its integrity, schema, digest, or semantic consistency claims.

- `Part II — Common structured representation > 9. Responsibility > 9.7 `UnsupportedFeature`` [310-313]
  Preview: The request refers to a known category that the current implementation or version does not support.

- `Part II — Common structured representation > 9. Responsibility > 9.8 `ExternalIntegration`` [314-317]
  Preview: A failure occurred outside semantic machine execution, such as observer delivery.

- `Part II — Common structured representation > 9. Responsibility > 9.9 `LibraryDefect`` [318-321]
  Preview: A validated implementation state violates a `mossignal` invariant.
  Symbols: `mossignal`

- `Part II — Common structured representation > 10. Delivery` [322-341]
  Preview: Catalogue entries declare one or more allowed delivery forms: Delivery is not inferred from severity.
  Symbols: `Error`, `Warning`, `ReportFinding`, `OperationFailure`
  Normative: MAY 1

- `Part II — Common structured representation > 11. Subjects` [342-382]
  Preview: The subject model MUST identify every problem-bearing category required by the other specifications.
  Normative: MUST NOT 1, MUST 3

- `Part II — Common structured representation > 12. Related subjects` [383-419]
  Preview: A related subject has a typed role: Representative roles include: Related-subject order is semantic only where the evidence schema explicitly declares it ordered, such as a cycle witness or graph path.

- `Part II — Common structured representation > 13. Typed evidence` [420-455]
  Preview: Every code defines exactly one evidence variant and its required fields.
  Symbols: `TimeEvidence`, `MigrationEvidence`
  Normative: MUST NOT 1, MUST 1, SHOULD 2, MAY 2

- `Part II — Common structured representation > 14. Evidence families` [456-845]
  Preview: The catalogue uses the following recurring evidence shapes.

- `Part II — Common structured representation > 14. Evidence families > 14.1 `ForeignArtifactEvidence`` [460-468]

- `Part II — Common structured representation > 14. Evidence families > 14.2 `KeyConflictEvidence`` [469-477]

- `Part II — Common structured representation > 14. Evidence families > 14.3 `MissingReferenceEvidence`` [478-486]

- `Part II — Common structured representation > 14. Evidence families > 14.4 `DirectionEvidence`` [487-495]

- `Part II — Common structured representation > 14. Evidence families > 14.5 `KindMismatchEvidence`` [496-504]

- `Part II — Common structured representation > 14. Evidence families > 14.6 `DriverConflictEvidence`` [505-512]

- `Part II — Common structured representation > 14. Evidence families > 14.7 `ArityEvidence`` [513-522]

- `Part II — Common structured representation > 14. Evidence families > 14.8 `ParameterEvidence`` [523-531]

- `Part II — Common structured representation > 14. Evidence families > 14.9 `StateSchemaEvidence`` [532-540]

- `Part II — Common structured representation > 14. Evidence families > 14.10 `ModuleSchemaEvidence`` [541-549]

- `Part II — Common structured representation > 14. Evidence families > 14.11 `HierarchyEvidence`` [550-557]

- `Part II — Common structured representation > 14. Evidence families > 14.12 `CurrentReactionCycleEvidence`` [558-566]

- `Part II — Common structured representation > 14. Evidence families > 14.13 `DependencySignatureEvidence`` [567-575]

- `Part II — Common structured representation > 14. Evidence families > 14.14 `StaticQualityEvidence`` [576-583]

- `Part II — Common structured representation > 14. Evidence families > 14.15 `LifecycleEvidence`` [584-593]

- `Part II — Common structured representation > 14. Evidence families > 14.16 `RevisionMismatchEvidence`` [594-603]

- `Part II — Common structured representation > 14. Evidence families > 14.17 `DigestMismatchEvidence`` [604-612]

- `Part II — Common structured representation > 14. Evidence families > 14.18 `DigestCollisionEvidence`` [613-622]

- `Part II — Common structured representation > 14. Evidence families > 14.19 `TimeEvidence`` [623-631]

- `Part II — Common structured representation > 14. Evidence families > 14.20 `InputObservationEvidence`` [632-641]

- `Part II — Common structured representation > 14. Evidence families > 14.21 `InputSchemaEvidence`` [642-650]

- `Part II — Common structured representation > 14. Evidence families > 14.22 `BindingEvidence`` [651-660]

- `Part II — Common structured representation > 14. Evidence families > 14.23 `StaleArtifactEvidence`` [661-669]

- `Part II — Common structured representation > 14. Evidence families > 14.24 `ConflictEvidence`` [670-680]

- `Part II — Common structured representation > 14. Evidence families > 14.25 `BudgetEvidence`` [681-690]

- `Part II — Common structured representation > 14. Evidence families > 14.26 `PatchEditEvidence`` [691-699]

- `Part II — Common structured representation > 14. Evidence families > 14.27 `MigrationEvidence`` [700-709]

- `Part II — Common structured representation > 14. Evidence families > 14.28 `SemanticLossEvidence`` [710-719]

- `Part II — Common structured representation > 14. Evidence families > 14.29 `CanonicalEncodingEvidence`` [720-749]
  Preview: Canonical violation kinds include at least:

- `Part II — Common structured representation > 14. Evidence families > 14.30 `VersionCompatibilityEvidence`` [750-760]

- `Part II — Common structured representation > 14. Evidence families > 14.31 `ArtifactIdentityEvidence`` [761-768]

- `Part II — Common structured representation > 14. Evidence families > 14.32 `PendingEventEvidence`` [769-779]

- `Part II — Common structured representation > 14. Evidence families > 14.33 `DiagnosticEpisodeEvidence`` [780-791]

- `Part II — Common structured representation > 14. Evidence families > 14.34 `ProvenanceEvidence`` [792-802]

- `Part II — Common structured representation > 14. Evidence families > 14.35 `ReplayEvidence`` [803-811]

- `Part II — Common structured representation > 14. Evidence families > 14.36 `StandardModuleEvidence`` [812-823]

- `Part II — Common structured representation > 14. Evidence families > 14.37 `ObserverEvidence`` [824-833]

- `Part II — Common structured representation > 14. Evidence families > 14.38 `InternalInvariantEvidence`` [834-845]

- `Part II — Common structured representation > 15. Suggestions` [846-872]
  Preview: Suggestions are closed, machine-readable actions.
  Normative: MUST NOT 1, MAY 1

- `Part III — Reports, failures, ordering, and identity` [873-1048]
  Preview: Validation, compilation, binding, and structural patch preparation use: Independent findings SHOULD be collected where safe.
  Symbols: `Error`, `Warning`, `Info`, `DiagnosticSet`, `ReportFailure`, `require_artifact`, `internal.diagnostic_evidence_conflict`, `PersistentEpisode`
  Normative: MUST NOT 3, MUST 2, SHOULD NOT 1, SHOULD 3, MAY 5

- `Part III — Reports, failures, ordering, and identity > 16. Reports` [875-895]
  Preview: Validation, compilation, binding, and structural patch preparation use: Independent findings SHOULD be collected where safe.
  Symbols: `Error`, `Warning`, `Info`, `ReportFailure`, `require_artifact`
  Normative: MUST 1, SHOULD NOT 1, SHOULD 1, MAY 1

- `Part III — Reports, failures, ordering, and identity > 17. Failure enums` [896-913]
  Preview: Operation-specific Rust failure enums remain useful and SHOULD retain ergonomic category matching.
  Normative: MUST 1, SHOULD 1, MAY 1

- `Part III — Reports, failures, ordering, and identity > 18. Deterministic ordering` [914-934]
  Preview: `DiagnosticSet` iteration order is canonical: 1.
  Symbols: `DiagnosticSet`, `Error`, `Warning`, `Info`
  Normative: MUST NOT 1, MAY 1

- `Part III — Reports, failures, ordering, and identity > 19. Condition identity and deduplication` [935-956]
  Preview: Every catalogue entry defines a condition discriminator.
  Symbols: `DiagnosticSet`, `internal.diagnostic_evidence_conflict`
  Normative: MUST NOT 1

- `Part III — Reports, failures, ordering, and identity > 20. Persistent episodes` [957-995]
  Preview: A code is episode-capable only when its catalogue entry permits `PersistentEpisode` delivery.
  Symbols: `PersistentEpisode`

- `Part III — Reports, failures, ordering, and identity > 21. Episode identity and migration` [996-1016]
  Preview: A fresh episode identity is derived from: The resulting opaque identifier is persisted rather than recomputed from private runtime positions.
  Normative: MUST NOT 1, MAY 1

- `Part III — Reports, failures, ordering, and identity > 22. Runtime occurrences` [1017-1028]
  Preview: Transient runtime diagnostics are committed occurrences attached to one reaction or outer transaction.

- `Part III — Reports, failures, ordering, and identity > 23. Context and reproducibility` [1029-1048]
  Preview: Problem records MAY carry a standard non-semantic context envelope containing: This envelope does not participate in condition identity unless a catalogue entry explicitly makes one field semantic.
  Normative: SHOULD 1, MAY 1

- `Part IV — Namespace rules` [1049-1081]
  Preview: The initial namespace set is: A namespace identifies the semantic domain of the condition, not necessarily the implementation layer that detected it.
  Symbols: `validation.*`, `internal.*`, `compilation.*`, `reconfiguration.*`

- `Part IV — Namespace rules > 24. Initial namespaces` [1051-1081]
  Preview: The initial namespace set is: A namespace identifies the semantic domain of the condition, not necessarily the implementation layer that detected it.
  Symbols: `validation.*`, `internal.*`, `compilation.*`, `reconfiguration.*`

- `Part V — Exhaustive initial catalogue` [1082-1454]
  Preview: The following tables are normative.
  Symbols: `InternalInvariantEvidence`, `StandardModuleEvidence`, `ReplayEvidence`, `ProvenanceEvidence`, `ParameterEvidence`, `MissingReferenceEvidence`, `StaticQualityEvidence`, `InputObservationEvidence`, `BindingEvidence`, `InputSchemaEvidence`, `PatchEditEvidence`, `DigestMismatchEvidence`, `ArtifactIdentityEvidence`, `MigrationEvidence`, `LifecycleEvidence`, `StaleArtifactEvidence`, `PendingEventEvidence`, `CanonicalEncodingEvidence`, `VersionCompatibilityEvidence`, `ForeignArtifactEvidence`, `ArityEvidence`, `ConflictEvidence`, `TimeEvidence`, `SemanticLossEvidence`, `DiagnosticEpisodeEvidence`, `ObserverEvidence`, `validation.*`, `KindMismatchEvidence`, `StateSchemaEvidence`, `ModuleSchemaEvidence`, `HierarchyEvidence`, `DependencySignatureEvidence`, `validation.constant_result_node`, `lifecycle.not_initialized`, `RevisionMismatchEvidence`, `runtime.time_overflow`, `runtime.budget_exceeded`, `BudgetEvidence`, `RejectTransaction`, `authoring.foreign_signal`, `authoring.foreign_network_artifact`, `validation.duplicate_key`, `KeyConflictEvidence`, `validation.missing_node`, `validation.missing_port`, `validation.missing_endpoint`, `validation.invalid_direction`, `DirectionEvidence`, `validation.signal_kind_mismatch`, `validation.unsupported_multiple_drivers`, `DriverConflictEvidence`, `validation.missing_required_input`, `validation.invalid_fixed_arity`, `validation.invalid_variadic_arity`, `Zip`, `validation.invalid_parameter`, `validation.invalid_timing_parameter`, `validation.invalid_initial_state`, `validation.incompatible_state_schema`, `validation.invalid_module_interface`, `validation.invalid_module_binding`, `validation.malformed_hierarchy`, `validation.hierarchy_cycle`, `validation.current_reaction_cycle`, `CurrentReactionCycleEvidence`, `validation.invalid_dependency_signature`, `validation.incomplete_dependency_signature`, `validation.incompatible_network_reference`, `validation.unsupported_node_kind`, `validation.duplicate_source`, `validation.empty_variadic_node`, `validation.unary_degenerate_node`, `validation.unreachable_output`, `validation.unused_input`, `validation.isolated_node`, `validation.redundant_connection`, `validation.empty_module`, `validation.deprecated_node_form`, `lifecycle.already_initialized`, `lifecycle.snapshot_required`, `InputSnapshot`, `lifecycle.delta_before_initialization`, `InputDelta`, `lifecycle.delta_required_after_initialization`, `runtime.stale_revision`, `runtime.stale_execution_state`, `runtime.time_not_strictly_increasing`, `runtime.invalid_time_subtraction`, `runtime.zero_span_not_allowed`, `runtime.pulse_count_overflow`, `runtime.policy_missing_limit`, `runtime.policy_invalid_limit`, `runtime.pulse_latch_conflict_retained`, `runtime.pulse_latch_conflict_rejected`, `runtime.level_latch_conflict_retained`, `RetainAndDiagnose`, `runtime.level_latch_conflict_rejected`, `input.unknown_endpoint`, `input.wrong_signal_kind`, `input.duplicate_observation`, `input.conflicting_observation`, `input.ambiguous_observation`, `input.missing_required_level`, `input.wrong_network`, `input.foreign_schema`, `input.stale_schema`, `input.removed_endpoint`, `input.new_level_requires_establish`, `input.establish_not_permitted`, `establish`, `input.target_schema_mismatch`, `binding.unknown_endpoint`, `binding.wrong_signal_kind`, `binding.duplicate_endpoint`, `binding.duplicate_external_key`, `binding.ambiguous_external_key`, `binding.missing_required_binding`, `binding.wrong_network`, `binding.stale_schema`, `inspection.unknown_subject`, `inspection.wrong_subject_kind`, `inspection.foreign_handle`, `inspection.stale_handle`, `inspection.foreign_plan`, `inspection.stale_plan`, `inspection.pending_event_not_found`, `inspection.unsupported_projection`, `explanation.unknown_subject`, `explanation.foreign_cause`, `explanation.invalid_cause`, `explanation.cause_not_retained`, `explanation.request_outside_retention`, `explanation.history_truncated`, `explanation.unsupported_request`, `reconfiguration.foreign_artifact`, `reconfiguration.duplicate_operation`, `reconfiguration.conflicting_edit`, `reconfiguration.invalid_replacement_key`, `reconfiguration.invalid_reassociation`, `reconfiguration.contradictory_hierarchy`, `reconfiguration.base_fingerprint_mismatch`, `reconfiguration.base_revision_mismatch`, `reconfiguration.unknown_base_subject`, `reconfiguration.non_injective_reassociation`, `reconfiguration.incompatible_migration_directive`, `reconfiguration.incomplete_temporal_migration_policy`, `reconfiguration.unsupported_cross_kind_migration`, `reconfiguration.ambiguous_event_migration`, `reconfiguration.conflicting_migrated_transitions`, `reconfiguration.invalid_target_input_schema`, `reconfiguration.conditional_semantic_loss`, `reconfiguration.unavoidable_semantic_loss`, `reconfiguration.empty_patch`, `reconfiguration.stale_prepared_patch`, `reconfiguration.target_input_schema_mismatch`, `reconfiguration.state_migration_rejected`, `reconfiguration.pending_event_migration_rejected`, `reconfiguration.require_preserve_failed`, `RequirePreserve`, `reconfiguration.episode_migration_rejected`, `reconfiguration.provenance_migration_rejected`, `reconfiguration.state_loss_rejected`, `RejectStateLoss`, `persistence.invalid_prefix`, `persistence.truncated_artifact`, `persistence.trailing_bytes`, `persistence.noncanonical_encoding`, `persistence.malformed_envelope`, `persistence.unknown_artifact_kind`, `persistence.unknown_schema_field`, `persistence.unknown_schema_variant`, `persistence.integrity_digest_mismatch`, `persistence.decode_limit_exceeded`, `persistence.unsupported_version`, `persistence.wrong_time_domain`, `TimeDomainId`, `persistence.semantic_migration_required`, `persistence.network_identity_mismatch`, `persistence.fingerprint_mismatch`, `persistence.topology_revision_mismatch`, `persistence.runtime_policy_mismatch`, `persistence.validation_claim_mismatch`, `persistence.lifecycle_shape_invalid`, `persistence.state_schema_mismatch`, `persistence.unknown_subject`, `persistence.pending_event_invalid`, `persistence.event_identity_state_invalid`, `persistence.diagnostic_episode_invalid`, `persistence.diagnostic_schema_invalid`, `persistence.settled_state_inconsistent`, `persistence.execution_digest_mismatch`, `persistence.observable_digest_mismatch`, `persistence.snapshot_digest_mismatch`, `persistence.digest_collision`, `DigestCollisionEvidence`, `persistence.provenance_missing_predecessor`, `persistence.provenance_digest_mismatch`, `CauseDigest`, `persistence.provenance_cycle`, `persistence.provenance_invalid_subject`, `persistence.provenance_invalid_role`, `persistence.provenance_incomplete_root_closure`, `persistence.provenance_conflicting_record`, `persistence.provenance_false_checkpoint`, `replay.starting_execution_digest_mismatch`, `replay.starting_observable_digest_mismatch`, `replay.expected_revision_mismatch`, `replay.runtime_policy_mismatch`, `replay.time_domain_mismatch`, `replay.network_fingerprint_mismatch`, `replay.target_fingerprint_mismatch`, `replay.patch_preparation_diverged`, `replay.resulting_execution_digest_mismatch`, `replay.resulting_observable_digest_mismatch`, `replay.recorded_result_mismatch`, `replay.frame_missing`, `replay.frame_reordered`, `replay.frame_duplicated`, `replay.chunk_sequence_mismatch`, `replay.chunk_link_mismatch`, `replay.logs_not_concatenable`, `replay.frame_failed`, `standard_module.unknown_id`, `standard_module.unsupported_version`, `standard_module.missing_parameter`, `standard_module.unexpected_parameter`, `standard_module.parameter_kind_mismatch`, `standard_module.invalid_parameter`, `standard_module.interface_mismatch`, `standard_module.expansion_mismatch`, `standard_module.noncanonical_internal_edit`, `standard_module.incompatible_version_migration`, `standard_module.internal_key_collision`, `standard_module.catalogue_invariant`, `LibraryDefect`, `standard_module.deprecated_alias`, `standard_module.empty_variadic`, `standard_module.unary_degenerate`, `standard_module.impossible_threshold`, `Exactly.threshold`, `Low`, `standard_module.constant_result`, `standard_module.duplicate_source`, `observer.cursor_stale`, `observer.resynchronization_required`, `observer.delivery_failed`, `internal.diagnostic_code_evidence_mismatch`, `internal.diagnostic_evidence_conflict`, `internal.compiled_dense_reference_out_of_bounds`, `internal.compiled_descriptor_kind_mismatch`, `internal.compiled_port_kind_mismatch`, `internal.compiled_connection_driver_invalid`, `internal.reaction_dependency_not_forward`, `internal.reaction_cycle_after_compilation`, `internal.stable_key_lookup_ambiguous`, `internal.endpoint_table_incomplete`, `internal.state_slot_family_mismatch`, `internal.state_slot_owner_mismatch`, `internal.multiple_state_successors`, `internal.pending_event_owner_invalid`, `internal.pending_deadline_not_future`, `internal.event_calendar_minimum_mismatch`, `internal.event_calendar_membership_mismatch`, `internal.event_key_reused`, `internal.region_partition_invalid`, `internal.migration_classification_incomplete`, `internal.migration_classification_duplicate`, `internal.diagnostic_episode_owner_invalid`, `internal.provenance_cycle`, `internal.provenance_root_unreachable`, `internal.provenance_subject_invalid`, `internal.failure_atomicity_violated`, `internal.machine_mutated_by_inspection`, `internal.incremental_reaction_divergence`, `internal.transaction_executor_divergence`, `internal.event_calendar_divergence`, `internal.reconfiguration_divergence`, `internal.region_divergence`, `internal.inspection_divergence`, `internal.forecast_divergence`, `internal.replay_divergence`, `internal.canonical_encoding_divergence`, `internal.persistence_projection_invalid`, `internal.canonical_digest_mismatch`, `internal.snapshot_round_trip_divergence`
  Normative: MUST 1

- `Part V — Exhaustive initial catalogue > 25. Authoring` [1096-1104]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `authoring.foreign_signal` | Error | CallerInput | Failure | `ForeignArtifactEvidence` | A builder-scoped signal from another builder was supplied.
  Symbols: `ForeignArtifactEvidence`, `authoring.foreign_signal`, `authoring.foreign_network_artifact`, `validation.*`

- `Part V — Exhaustive initial catalogue > 26. Validation — blocking conditions` [1105-1132]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `validation.duplicate_key` | Error | CallerInput | Report, Failure | `KeyConflictEvidence` | Two subjects claim one stable key in a scope requiring uniqueness.
  Symbols: `MissingReferenceEvidence`, `ParameterEvidence`, `ArityEvidence`, `ModuleSchemaEvidence`, `HierarchyEvidence`, `DependencySignatureEvidence`, `validation.duplicate_key`, `KeyConflictEvidence`, `validation.missing_node`, `validation.missing_port`, `validation.missing_endpoint`, `validation.invalid_direction`, `DirectionEvidence`, `validation.signal_kind_mismatch`, `KindMismatchEvidence`, `validation.unsupported_multiple_drivers`, `DriverConflictEvidence`, `validation.missing_required_input`, `validation.invalid_fixed_arity`, `validation.invalid_variadic_arity`, `Zip`, `validation.invalid_parameter`, `validation.invalid_timing_parameter`, `validation.invalid_initial_state`, `validation.incompatible_state_schema`, `StateSchemaEvidence`, `validation.invalid_module_interface`, `validation.invalid_module_binding`, `validation.malformed_hierarchy`, `validation.hierarchy_cycle`, `validation.current_reaction_cycle`, `CurrentReactionCycleEvidence`, `validation.invalid_dependency_signature`, `validation.incomplete_dependency_signature`, `validation.incompatible_network_reference`, `ForeignArtifactEvidence`, `validation.unsupported_node_kind`

- `Part V — Exhaustive initial catalogue > 27. Validation — non-blocking conditions` [1133-1149]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `validation.duplicate_source` | Warning | CallerInput | Report | `StaticQualityEvidence` | One upstream source feeds several ports of the same node where multiplicity remains port-based.
  Symbols: `StaticQualityEvidence`, `ArityEvidence`, `validation.constant_result_node`, `validation.duplicate_source`, `validation.empty_variadic_node`, `validation.unary_degenerate_node`, `validation.unreachable_output`, `validation.unused_input`, `validation.isolated_node`, `validation.redundant_connection`, `validation.empty_module`, `validation.deprecated_node_form`

- `Part V — Exhaustive initial catalogue > 28. Lifecycle` [1150-1159]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `lifecycle.not_initialized` | Error | CallerInput | Failure | `LifecycleEvidence` | Runtime state, schedule, pending work, or explanation was requested before initialization.
  Symbols: `LifecycleEvidence`, `lifecycle.not_initialized`, `lifecycle.already_initialized`, `lifecycle.snapshot_required`, `InputSnapshot`, `lifecycle.delta_before_initialization`, `InputDelta`, `lifecycle.delta_required_after_initialization`

- `Part V — Exhaustive initial catalogue > 29. Runtime values, time, policy, and semantic rejection` [1160-1178]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `runtime.stale_revision` | Error | Compatibility | Failure | `RevisionMismatchEvidence` | An operation expected a different topology revision from the machine's current revision.
  Symbols: `ParameterEvidence`, `ConflictEvidence`, `TimeEvidence`, `RejectTransaction`, `runtime.stale_revision`, `RevisionMismatchEvidence`, `runtime.stale_execution_state`, `DigestMismatchEvidence`, `runtime.time_not_strictly_increasing`, `runtime.time_overflow`, `runtime.invalid_time_subtraction`, `runtime.zero_span_not_allowed`, `runtime.pulse_count_overflow`, `runtime.policy_missing_limit`, `runtime.policy_invalid_limit`, `runtime.budget_exceeded`, `BudgetEvidence`, `runtime.pulse_latch_conflict_retained`, `runtime.pulse_latch_conflict_rejected`, `runtime.level_latch_conflict_retained`, `RetainAndDiagnose`, `runtime.level_latch_conflict_rejected`

- `Part V — Exhaustive initial catalogue > 30. Input construction and projection` [1179-1196]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `input.unknown_endpoint` | Error | CallerInput | Failure | `InputObservationEvidence` | An input artifact references an endpoint absent from its schema.
  Symbols: `InputObservationEvidence`, `InputSchemaEvidence`, `input.unknown_endpoint`, `input.wrong_signal_kind`, `input.duplicate_observation`, `input.conflicting_observation`, `input.ambiguous_observation`, `input.missing_required_level`, `input.wrong_network`, `input.foreign_schema`, `input.stale_schema`, `input.removed_endpoint`, `input.new_level_requires_establish`, `input.establish_not_permitted`, `establish`, `input.target_schema_mismatch`

- `Part V — Exhaustive initial catalogue > 31. Bindings` [1197-1209]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `binding.unknown_endpoint` | Error | CallerInput | Report, Failure | `BindingEvidence` | A binding references an endpoint absent from the compiled network.
  Symbols: `BindingEvidence`, `binding.unknown_endpoint`, `binding.wrong_signal_kind`, `binding.duplicate_endpoint`, `binding.duplicate_external_key`, `binding.ambiguous_external_key`, `binding.missing_required_binding`, `binding.wrong_network`, `binding.stale_schema`

- `Part V — Exhaustive initial catalogue > 32. Inspection` [1210-1224]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `inspection.unknown_subject` | Error | CallerInput | Failure | `MissingReferenceEvidence` | The requested stable subject does not exist in the inspected artifact.
  Symbols: `StaleArtifactEvidence`, `inspection.unknown_subject`, `MissingReferenceEvidence`, `inspection.wrong_subject_kind`, `KindMismatchEvidence`, `inspection.foreign_handle`, `inspection.stale_handle`, `inspection.foreign_plan`, `inspection.stale_plan`, `inspection.pending_event_not_found`, `PendingEventEvidence`, `inspection.unsupported_projection`, `ParameterEvidence`, `lifecycle.not_initialized`

- `Part V — Exhaustive initial catalogue > 33. Explanation and provenance access` [1225-1236]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `explanation.unknown_subject` | Error | CallerInput | Failure | `MissingReferenceEvidence` | The requested explainable subject does not exist.
  Symbols: `ProvenanceEvidence`, `explanation.unknown_subject`, `MissingReferenceEvidence`, `explanation.foreign_cause`, `explanation.invalid_cause`, `explanation.cause_not_retained`, `explanation.request_outside_retention`, `explanation.history_truncated`, `explanation.unsupported_request`, `ParameterEvidence`

- `Part V — Exhaustive initial catalogue > 34. Reconfiguration — patch construction and preparation` [1237-1262]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `reconfiguration.foreign_artifact` | Error | CallerInput | Failure | `ForeignArtifactEvidence` | A patch operation contains an artifact from another network or time domain.
  Symbols: `PatchEditEvidence`, `MigrationEvidence`, `PendingEventEvidence`, `SemanticLossEvidence`, `reconfiguration.foreign_artifact`, `ForeignArtifactEvidence`, `reconfiguration.duplicate_operation`, `reconfiguration.conflicting_edit`, `reconfiguration.invalid_replacement_key`, `reconfiguration.invalid_reassociation`, `reconfiguration.contradictory_hierarchy`, `reconfiguration.base_fingerprint_mismatch`, `ArtifactIdentityEvidence`, `reconfiguration.base_revision_mismatch`, `RevisionMismatchEvidence`, `reconfiguration.unknown_base_subject`, `MissingReferenceEvidence`, `reconfiguration.non_injective_reassociation`, `reconfiguration.incompatible_migration_directive`, `reconfiguration.incomplete_temporal_migration_policy`, `reconfiguration.unsupported_cross_kind_migration`, `reconfiguration.ambiguous_event_migration`, `reconfiguration.conflicting_migrated_transitions`, `reconfiguration.invalid_target_input_schema`, `InputSchemaEvidence`, `reconfiguration.conditional_semantic_loss`, `reconfiguration.unavoidable_semantic_loss`, `reconfiguration.empty_patch`, `validation.*`

- `Part V — Exhaustive initial catalogue > 35. Reconfiguration — finalization` [1263-1277]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `reconfiguration.stale_prepared_patch` | Error | Compatibility | Failure | `StaleArtifactEvidence` | A prepared patch no longer matches the machine's exact base revision or fingerprint.
  Symbols: `MigrationEvidence`, `reconfiguration.stale_prepared_patch`, `StaleArtifactEvidence`, `reconfiguration.target_input_schema_mismatch`, `InputSchemaEvidence`, `reconfiguration.state_migration_rejected`, `reconfiguration.pending_event_migration_rejected`, `reconfiguration.require_preserve_failed`, `RequirePreserve`, `reconfiguration.episode_migration_rejected`, `DiagnosticEpisodeEvidence`, `reconfiguration.provenance_migration_rejected`, `ProvenanceEvidence`, `reconfiguration.state_loss_rejected`, `SemanticLossEvidence`, `RejectStateLoss`, `runtime.time_overflow`, `runtime.budget_exceeded`

- `Part V — Exhaustive initial catalogue > 36. Persistence — canonical decoding and envelopes` [1278-1295]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `persistence.invalid_prefix` | Error | CorruptData | Failure | `CanonicalEncodingEvidence` | Standalone bytes do not begin with the required artifact prefix.
  Symbols: `CanonicalEncodingEvidence`, `VersionCompatibilityEvidence`, `persistence.invalid_prefix`, `persistence.truncated_artifact`, `persistence.trailing_bytes`, `persistence.noncanonical_encoding`, `persistence.malformed_envelope`, `persistence.unknown_artifact_kind`, `persistence.unknown_schema_field`, `persistence.unknown_schema_variant`, `persistence.integrity_digest_mismatch`, `DigestMismatchEvidence`, `persistence.decode_limit_exceeded`, `BudgetEvidence`, `persistence.unsupported_version`, `persistence.wrong_time_domain`, `ArtifactIdentityEvidence`, `TimeDomainId`, `persistence.semantic_migration_required`

- `Part V — Exhaustive initial catalogue > 37. Persistence — restoration identity and semantic consistency` [1296-1317]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `persistence.network_identity_mismatch` | Error | Compatibility | Failure | `ArtifactIdentityEvidence` | The artifact and supplied compiled network have different network identities.
  Symbols: `ArtifactIdentityEvidence`, `DigestMismatchEvidence`, `PendingEventEvidence`, `DiagnosticEpisodeEvidence`, `persistence.network_identity_mismatch`, `persistence.fingerprint_mismatch`, `persistence.topology_revision_mismatch`, `persistence.runtime_policy_mismatch`, `persistence.validation_claim_mismatch`, `persistence.lifecycle_shape_invalid`, `ParameterEvidence`, `persistence.state_schema_mismatch`, `StateSchemaEvidence`, `persistence.unknown_subject`, `MissingReferenceEvidence`, `persistence.pending_event_invalid`, `persistence.event_identity_state_invalid`, `persistence.diagnostic_episode_invalid`, `persistence.diagnostic_schema_invalid`, `persistence.settled_state_inconsistent`, `InternalInvariantEvidence`, `persistence.execution_digest_mismatch`, `persistence.observable_digest_mismatch`, `persistence.snapshot_digest_mismatch`, `persistence.digest_collision`, `DigestCollisionEvidence`

- `Part V — Exhaustive initial catalogue > 38. Persistence — provenance validation` [1318-1330]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `persistence.provenance_missing_predecessor` | Error | CorruptData | Failure | `ProvenanceEvidence` | A provenance record references an unavailable predecessor.
  Symbols: `ProvenanceEvidence`, `persistence.provenance_missing_predecessor`, `persistence.provenance_digest_mismatch`, `CauseDigest`, `persistence.provenance_cycle`, `persistence.provenance_invalid_subject`, `persistence.provenance_invalid_role`, `persistence.provenance_incomplete_root_closure`, `persistence.provenance_conflicting_record`, `persistence.provenance_false_checkpoint`

- `Part V — Exhaustive initial catalogue > 39. Replay` [1331-1354]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `replay.starting_execution_digest_mismatch` | Error | Compatibility | Failure | `ReplayEvidence` | The machine does not match the replay log's required starting execution digest.
  Symbols: `ReplayEvidence`, `replay.starting_execution_digest_mismatch`, `replay.starting_observable_digest_mismatch`, `replay.expected_revision_mismatch`, `replay.runtime_policy_mismatch`, `replay.time_domain_mismatch`, `replay.network_fingerprint_mismatch`, `replay.target_fingerprint_mismatch`, `replay.patch_preparation_diverged`, `replay.resulting_execution_digest_mismatch`, `replay.resulting_observable_digest_mismatch`, `replay.recorded_result_mismatch`, `replay.frame_missing`, `replay.frame_reordered`, `replay.frame_duplicated`, `replay.chunk_sequence_mismatch`, `replay.chunk_link_mismatch`, `replay.logs_not_concatenable`, `replay.frame_failed`

- `Part V — Exhaustive initial catalogue > 40. Standard modules — blocking conditions` [1355-1375]
  Preview: These identifiers are inherited from the Standard Module Catalogue.
  Symbols: `StandardModuleEvidence`, `standard_module.unknown_id`, `standard_module.unsupported_version`, `standard_module.missing_parameter`, `standard_module.unexpected_parameter`, `standard_module.parameter_kind_mismatch`, `standard_module.invalid_parameter`, `standard_module.interface_mismatch`, `standard_module.expansion_mismatch`, `standard_module.noncanonical_internal_edit`, `standard_module.incompatible_version_migration`, `standard_module.internal_key_collision`, `standard_module.catalogue_invariant`, `LibraryDefect`
  Normative: MUST 1

- `Part V — Exhaustive initial catalogue > 41. Standard modules — non-blocking conditions` [1376-1388]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `standard_module.deprecated_alias` | Warning | Compatibility | Report | `StandardModuleEvidence` | An authoring alias is deprecated.
  Symbols: `StandardModuleEvidence`, `standard_module.deprecated_alias`, `standard_module.empty_variadic`, `standard_module.unary_degenerate`, `standard_module.impossible_threshold`, `Exactly.threshold`, `Low`, `standard_module.constant_result`, `standard_module.duplicate_source`

- `Part V — Exhaustive initial catalogue > 42. Observer integration` [1389-1398]
  Preview: The initial machine core does not require an observer implementation, but these codes are reserved for the specified observer boundary.
  Symbols: `ObserverEvidence`, `observer.cursor_stale`, `observer.resynchronization_required`, `observer.delivery_failed`

- `Part V — Exhaustive initial catalogue > 43. Internal defects — static compiled invariants` [1399-1415]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `internal.diagnostic_code_evidence_mismatch` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | A problem code is paired with the wrong typed evidence variant.
  Symbols: `InternalInvariantEvidence`, `internal.diagnostic_code_evidence_mismatch`, `internal.diagnostic_evidence_conflict`, `internal.compiled_dense_reference_out_of_bounds`, `internal.compiled_descriptor_kind_mismatch`, `internal.compiled_port_kind_mismatch`, `internal.compiled_connection_driver_invalid`, `internal.reaction_dependency_not_forward`, `internal.reaction_cycle_after_compilation`, `internal.stable_key_lookup_ambiguous`, `internal.endpoint_table_incomplete`, `internal.state_slot_family_mismatch`, `internal.state_slot_owner_mismatch`

- `Part V — Exhaustive initial catalogue > 44. Internal defects — runtime state and transaction invariants` [1416-1435]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `internal.multiple_state_successors` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | One state cell received more than one proposed successor in one reaction.
  Symbols: `InternalInvariantEvidence`, `internal.multiple_state_successors`, `internal.pending_event_owner_invalid`, `internal.pending_deadline_not_future`, `internal.event_calendar_minimum_mismatch`, `internal.event_calendar_membership_mismatch`, `internal.event_key_reused`, `internal.region_partition_invalid`, `internal.migration_classification_incomplete`, `internal.migration_classification_duplicate`, `internal.diagnostic_episode_owner_invalid`, `internal.provenance_cycle`, `internal.provenance_root_unreachable`, `internal.provenance_subject_invalid`, `internal.failure_atomicity_violated`, `internal.machine_mutated_by_inspection`

- `Part V — Exhaustive initial catalogue > 45. Internal defects — reference-path divergence` [1436-1454]
  Preview: | Code | Severity | Responsibility | Delivery | Evidence | Meaning | |---|---|---|---|---|---| | `internal.incremental_reaction_divergence` | Error | LibraryDefect | Defect | `InternalInvariantEvidence` | Incremental and full topological reaction evaluation differ.
  Symbols: `InternalInvariantEvidence`, `internal.incremental_reaction_divergence`, `internal.transaction_executor_divergence`, `internal.event_calendar_divergence`, `internal.reconfiguration_divergence`, `internal.region_divergence`, `internal.inspection_divergence`, `internal.forecast_divergence`, `internal.replay_divergence`, `internal.canonical_encoding_divergence`, `internal.persistence_projection_invalid`, `internal.canonical_digest_mismatch`, `internal.snapshot_round_trip_divergence`

- `Part VI — Rust API integration` [1455-1599]
  Preview: The concrete Rust API MUST provide exact mappings broadly equivalent to the following.
  Symbols: `TransactionResult`, `RuntimeFailure`, `Unknown`, `#[non_exhaustive]`
  Normative: MUST NOT 1, MUST 1, SHOULD 1

- `Part VI — Rust API integration > 46. Required mapping by API family` [1457-1579]
  Preview: The concrete Rust API MUST provide exact mappings broadly equivalent to the following.
  Symbols: `TransactionResult`, `RuntimeFailure`
  Normative: MUST 1

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.1 Authoring and validation` [1461-1474]

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.2 Runtime policy and values` [1475-1492]

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.3 Inputs and bindings` [1493-1505]

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.4 Runtime application` [1506-1520]
  Preview: Runtime occurrences and episode events appear in a successful `TransactionResult`, not in `RuntimeFailure`.
  Symbols: `TransactionResult`, `RuntimeFailure`

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.5 Inspection and explanation` [1521-1534]

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.6 Reconfiguration` [1535-1551]

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.7 Persistence and replay` [1552-1569]

- `Part VI — Rust API integration > 46. Required mapping by API family > 46.8 Internal defects` [1570-1579]

- `Part VI — Rust API integration > 47. No arbitrary fallback strings` [1580-1591]
  Preview: Public failure enums MUST NOT contain a fallback variant whose only authoritative content is: An `Unknown` variant may exist for forward-compatible foreign-language bindings only when it retains the complete unknown code and canonical raw evidence.
  Symbols: `Unknown`
  Normative: MUST NOT 1

- `Part VI — Rust API integration > 48. Non-exhaustive enums` [1592-1599]
  Preview: Failure and evidence enums SHOULD remain `#[non_exhaustive]` while the diagnostic schema is experimental.
  Symbols: `#[non_exhaustive]`
  Normative: SHOULD 1

- `Part VII — Persistence and compatibility` [1600-1667]
  Preview: `DiagnosticSchemaVersion` governs: Rendered wording and localization are excluded.
  Symbols: `DiagnosticSchemaVersion`, `persistence.diagnostic_schema_invalid`, `persistence.unsupported_version`
  Normative: MUST NOT 1, MUST 1, MAY 1

- `Part VII — Persistence and compatibility > 49. Diagnostic schema version` [1602-1620]
  Preview: `DiagnosticSchemaVersion` governs: Rendered wording and localization are excluded.
  Symbols: `DiagnosticSchemaVersion`

- `Part VII — Persistence and compatibility > 50. Canonical persisted form` [1621-1642]
  Preview: Persisted problem records contain: Severity and responsibility are stored for audit readability but MUST be verified against the code registry during decoding.
  Symbols: `persistence.diagnostic_schema_invalid`
  Normative: MUST 1, MAY 1

- `Part VII — Persistence and compatibility > 51. Unknown codes` [1643-1650]
  Preview: Under a known supported `DiagnosticSchemaVersion`, an unknown code or evidence variant is invalid persisted data.
  Symbols: `DiagnosticSchemaVersion`, `persistence.unsupported_version`
  Normative: MUST NOT 1

- `Part VII — Persistence and compatibility > 52. Episode persistence` [1651-1667]
  Preview: Active episode state participates in: - execution-state digests; - snapshots; - restoration; - replay; - reconfiguration migration; - current inspection.

- `Part VIII — Verification obligations` [1668-1818]
  Preview: The implementation MUST maintain a machine-readable registry containing every catalogue entry and its normative metadata.
  Symbols: `internal.diagnostic_evidence_conflict`, `DiagnosticSchemaVersion`
  Normative: MUST NOT 1, MUST 3

- `Part VIII — Verification obligations > 53. Catalogue completeness` [1670-1683]
  Preview: The implementation MUST maintain a machine-readable registry containing every catalogue entry and its normative metadata.
  Normative: MUST 1

- `Part VIII — Verification obligations > 54. Structural comparison` [1684-1700]
  Preview: Tests compare problem records by: Rendered prose is excluded from semantic equality.

- `Part VIII — Verification obligations > 55. Golden catalogue` [1701-1716]
  Preview: CI MUST generate or validate: Generated output must be deterministic and reviewable.
  Normative: MUST 1

- `Part VIII — Verification obligations > 56. Determinism and permutation tests` [1717-1732]
  Preview: Tests MUST permute: Canonical diagnostic sets must remain identical.
  Normative: MUST 1

- `Part VIII — Verification obligations > 57. Deduplication tests` [1733-1747]
  Preview: For every deduplicating code, tests must cover: Contradictory scalar evidence must produce `internal.diagnostic_evidence_conflict` in verification configurations.
  Symbols: `internal.diagnostic_evidence_conflict`

- `Part VIII — Verification obligations > 58. Episode tests` [1748-1765]
  Preview: Every episode-capable code must cover:

- `Part VIII — Verification obligations > 59. Internal-defect tests` [1766-1780]
  Preview: Every required debug invariant check must have at least one test demonstrating that deliberate corruption or an injected candidate/reference mismatch produces the exact internal code and evidence variant.

- `Part VIII — Verification obligations > 60. Failure atomicity` [1781-1800]
  Preview: For every operation-failure code capable of arising after candidate state construction begins, tests must compare the complete semantic machine before and after rejection.

- `Part VIII — Verification obligations > 61. Release gates` [1801-1818]
  Preview: A release claiming a feature MUST NOT proceed when: - a documented failure condition lacks a catalogue entry; - a public leaf failure emits only prose; - a code can be paired with incompatible evidence; - diagnostic order is nondeterministic; - an episode repeats unchanged warnings; - a structured rejection partially mutates machine state; - a named internal invariant can fail without a structured defect; - a current-schema persisted diagnostic does not round-trip canonically; - an intentional experimental schema change was made without advancing `DiagnosticSchemaVersion` and updating current golden data.
  Symbols: `DiagnosticSchemaVersion`
  Normative: MUST NOT 1

- `Part IX — Implementation guidance` [1819-1898]
  Preview: The implementation MUST represent the catalogue once in an authoritative declarative registry.
  Symbols: `internal.*`, `Level`, `Pulse`
  Normative: MUST NOT 1, MUST 4, SHOULD 2, MAY 2

- `Part IX — Implementation guidance > 62. Authoritative registry` [1821-1862]
  Preview: The implementation MUST represent the catalogue once in an authoritative declarative registry.
  Normative: MUST 4, SHOULD 1

- `Part IX — Implementation guidance > 63. Rendering` [1863-1880]
  Preview: Rendering receives only the structured problem record and presentation context.
  Normative: MUST NOT 1, MAY 1

- `Part IX — Implementation guidance > 64. Development diagnostics` [1881-1888]
  Preview: Debug and test configurations SHOULD enable expensive checks corresponding to the `internal.*` catalogue.
  Symbols: `internal.*`
  Normative: SHOULD 1, MAY 1

- `Part IX — Implementation guidance > 65. No diagnostic signal semantics` [1889-1898]
  Preview: Problem records are not `Level` or `Pulse` signals.
  Symbols: `Level`, `Pulse`

- `Summary` [1899-1921]
  Preview: The diagnostic system is one coherent machine-readable account of every significant failure and warning in `mossignal`.
  Symbols: `mossignal`
