.PHONY: fmt fmt-check compile-check static-guardrails-check clippy test nextest \
	nextest-quiet doctest doc-check deny acceptance-record-check check-dev \
	check-final doc-index doc-index-check run

QUIET_CHECK = python3 scripts/run_check.py

fmt:
	cargo fmt --all

fmt-check:
	@$(QUIET_CHECK) formatting cargo fmt --all --check

compile-check:
	@$(QUIET_CHECK) compilation cargo check --workspace --all-targets --all-features --quiet

static-guardrails-check:
	@$(QUIET_CHECK) static-guardrails python3 scripts/check_static_guardrails.py

clippy:
	@$(QUIET_CHECK) clippy cargo clippy --workspace --all-targets --all-features --quiet -- -D warnings

test:
	cargo test --workspace --all-targets

nextest:
	cargo nextest run --workspace --all-features

nextest-quiet:
	@$(QUIET_CHECK) tests cargo nextest run --workspace --all-features \
		--failure-output final --success-output never --status-level fail --final-status-level fail

doctest:
	@$(QUIET_CHECK) doctests cargo test --workspace --doc --quiet

doc-check:
	@$(QUIET_CHECK) documentation env RUSTDOCFLAGS=-Dwarnings \
		cargo doc --workspace --all-features --no-deps --quiet

deny:
	@$(QUIET_CHECK) dependency-policy cargo deny check advisories bans sources

doc-index:
	python3 scripts/generate_docs_index.py

doc-index-check:
	@$(QUIET_CHECK) documentation-index-tests python3 scripts/test_generate_docs_index.py
	@$(QUIET_CHECK) documentation-index python3 scripts/generate_docs_index.py --check

acceptance-record-check:
	@if [ -z "$(BEAD_ID)" ]; then \
		echo "ERROR: set BEAD_ID, for example: make acceptance-record-check BEAD_ID=ms-123" >&2; \
		exit 2; \
	fi
	@$(QUIET_CHECK) acceptance-record python3 scripts/check_acceptance_record.py "$(BEAD_ID)"
	@echo "OK: acceptance record is synchronized"

# Keep this gate fast enough to run repeatedly during implementation.
check-dev: fmt-check compile-check nextest-quiet
	@echo "OK: development checks passed"

# Add new finite repository-wide checks here when their underlying facilities
# exist. Long fuzz campaigns and other scheduled verification remain separate.
check-final: fmt-check compile-check static-guardrails-check clippy nextest-quiet doctest doc-check deny doc-index-check
	@echo "OK: final checks passed"

run:
	cargo run -p mossignal
