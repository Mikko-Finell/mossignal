.PHONY: fmt fmt-check compile-check static-guardrails-check clippy test nextest \
	nextest-quiet contract-tools-test doctest doc-check deny acceptance-record-check \
	setup check-dev check-final run

UV_RUN = uv run --locked
PYTHON = $(UV_RUN) python
QUIET_CHECK = $(PYTHON) scripts/run_check.py

setup:
	@uv sync --locked
	@echo "OK: Python tooling is synchronized"

fmt:
	cargo fmt --all

fmt-check:
	@$(QUIET_CHECK) formatting cargo fmt --all --check

compile-check:
	@$(QUIET_CHECK) compilation cargo check --workspace --all-targets --all-features --quiet

static-guardrails-check:
	@$(QUIET_CHECK) static-guardrails $(PYTHON) scripts/check_static_guardrails.py

clippy:
	@$(QUIET_CHECK) clippy cargo clippy --workspace --all-targets --all-features --quiet -- -D warnings

test:
	cargo test --workspace --all-targets

nextest:
	cargo nextest run --workspace --all-features

nextest-quiet:
	@$(QUIET_CHECK) tests cargo nextest run --workspace --all-features \
		--failure-output final --success-output never --status-level fail --final-status-level fail

contract-tools-test:
	@$(QUIET_CHECK) contract-tools $(PYTHON) -m unittest scripts/test_contracts.py

doctest:
	@$(QUIET_CHECK) doctests cargo test --workspace --doc --quiet

doc-check:
	@$(QUIET_CHECK) documentation env RUSTDOCFLAGS=-Dwarnings \
		cargo doc --workspace --all-features --no-deps --quiet

deny:
	@$(QUIET_CHECK) dependency-policy cargo deny check advisories bans sources

acceptance-record-check:
	@if [ -z "$(BEAD_ID)" ]; then \
		echo "ERROR: set BEAD_ID, for example: make acceptance-record-check BEAD_ID=ms-123" >&2; \
		exit 2; \
	fi
	@$(QUIET_CHECK) acceptance-record $(PYTHON) scripts/check_acceptance_record.py "$(BEAD_ID)"
	@echo "OK: acceptance record is synchronized"

# Keep this gate fast enough to run repeatedly during implementation.
check-dev: fmt-check compile-check nextest-quiet contract-tools-test
	@echo "OK: development checks passed"

# Add new finite repository-wide checks here when their underlying facilities
# exist. Long fuzz campaigns and other scheduled verification remain separate.
check-final: fmt-check compile-check static-guardrails-check clippy nextest-quiet contract-tools-test doctest doc-check deny
	@echo "OK: final checks passed"

run:
	cargo run -p mossignal
