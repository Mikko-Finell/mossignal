#!/usr/bin/env python3
"""Focused tests for the read-only specification-contract utility."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("contracts.py")
SPEC = importlib.util.spec_from_file_location("contracts", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
contracts = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = contracts
SPEC.loader.exec_module(contracts)


class ContractsToolTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.original_root = contracts.REPOSITORY_ROOT
        self.original_contracts_directory = contracts.CONTRACTS_DIRECTORY
        contracts.REPOSITORY_ROOT = self.root
        contracts.CONTRACTS_DIRECTORY = self.root / "docs/specs/contracts"
        contracts.CONTRACTS_DIRECTORY.mkdir(parents=True)

    def tearDown(self) -> None:
        contracts.REPOSITORY_ROOT = self.original_root
        contracts.CONTRACTS_DIRECTORY = self.original_contracts_directory
        self.temporary_directory.cleanup()

    def write_document(self, content: str) -> Path:
        path = self.root / "docs/specs/example.md"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def write_contract(self, name: str, source: dict[str, object], status: str = "reviewed") -> Path:
        path = contracts.CONTRACTS_DIRECTORY / name
        document = {
            "id": "mossignal.example.subject",
            "title": "Example subject",
            "status": status,
            "summary": "Example summary.",
            "aliases": ["example"],
            "requirements": [
                {"id": "ordered-value", "sources": ["example_source"]},
            ],
            "recommendations": [
                {"id": "advised-value", "sources": ["example_source"]},
            ],
            "sources": {"example_source": source},
        }
        with path.open("w", encoding="utf-8") as output:
            contracts.require_yaml().safe_dump(document, output, sort_keys=False)
        return path

    def source(
        self,
        reviewed_hash: str | None = None,
        heading_path: list[str] | None = None,
    ) -> dict[str, object]:
        result: dict[str, object] = {
            "document": "docs/specs/example.md",
            "heading_path": heading_path or ["Part", "Subject"],
        }
        if reviewed_hash is not None:
            result["reviewed_hash"] = reviewed_hash
        return result

    def capture(self, function: object, *args: object) -> tuple[int, str]:
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = function(*args)
        return result, output.getvalue()

    def test_catalog_ignores_template(self) -> None:
        (contracts.CONTRACTS_DIRECTORY / "_template.yaml").write_text("id: template\n", encoding="utf-8")
        self.write_contract("example.yaml", self.source())

        _, output = self.capture(contracts.catalog)

        self.assertIn("mossignal.example.subject", output)
        self.assertNotIn("template", output)

    def test_exact_heading_path_includes_nested_subsections(self) -> None:
        document = self.write_document("# Part\nintro\n## Subject\nbody  \n### Nested\nchild\n## Other\nignored\n")

        section = contracts.resolve_section(document, ["Part", "Subject"])

        self.assertEqual(section, "## Subject\nbody  \n### Nested\nchild\n")
        self.assertEqual(
            contracts.normalize_section(section),
            b"## Subject\nbody\n### Nested\nchild\n",
        )

    def test_fenced_heading_like_lines_are_not_headings(self) -> None:
        document = self.write_document(
            "# Part\n"
            "## Public API\n"
            "```text\n"
            "### Generated example\n"
            "```\n"
            "### Stable keys\n"
            "nested body\n"
            "## Stable keys\n"
            "actual body\n"
            "~~~markdown\n"
            "## Stable keys\n"
            "~~~\n"
        )

        headings = contracts.parse_headings(document.read_text(encoding="utf-8"))
        stable_keys = [heading.path for heading in headings if heading.text == "Stable keys"]

        self.assertEqual(
            stable_keys,
            [("Part", "Public API", "Stable keys"), ("Part", "Stable keys")],
        )
        self.assertEqual(
            contracts.resolve_section(document, ["Part", "Stable keys"]),
            "## Stable keys\nactual body\n~~~markdown\n## Stable keys\n~~~\n",
        )

    def test_status_reports_unchanged_and_affected_rules(self) -> None:
        document = self.write_document("# Part\n## Subject\nbody\n")
        current_hash = contracts.section_hash(contracts.resolve_section(document, ["Part", "Subject"]))
        contract = self.write_contract("example.yaml", self.source(current_hash))

        _, output = self.capture(contracts.status, [contract])

        self.assertIn("status: unchanged", output)
        self.assertIn("- ordered-value", output)
        self.assertIn("- advised-value", output)

    def test_status_reports_changed_missing_ambiguous_and_unfingerprinted(self) -> None:
        self.write_document("# Part\n## Subject\nnew\n## Subject\nsecond\n")
        changed = self.source("sha256:" + "0" * 64)
        contract = self.write_contract("example.yaml", changed)
        contract_document = contracts.load_contract(contract)
        source = contract_document["sources"]["example_source"]
        self.assertEqual(contracts.inspect_source(contract_document, "example_source", source)[0], "ambiguous")

        self.write_document("# Part\n## Subject\nnew\n")
        self.assertEqual(contracts.inspect_source(contract_document, "example_source", source)[0], "changed")
        source["heading_path"] = ["Part", "Missing"]
        self.assertEqual(contracts.inspect_source(contract_document, "example_source", source)[0], "missing")
        source["heading_path"] = ["Part", "Subject"]
        source.pop("reviewed_hash")
        self.assertEqual(contracts.inspect_source(contract_document, "example_source", source)[0], "not_fingerprinted")

    def test_fingerprint_has_no_file_mutation(self) -> None:
        document = self.write_document("# Part\r\n## Subject\r\nbody\t\r\n")
        contract = self.write_contract("example.yaml", self.source())
        before = {path: hashlib.sha256(path.read_bytes()).hexdigest() for path in self.root.rglob("*") if path.is_file()}

        _, output = self.capture(contracts.fingerprint, [contract])

        after = {path: hashlib.sha256(path.read_bytes()).hexdigest() for path in self.root.rglob("*") if path.is_file()}
        self.assertEqual(before, after)
        self.assertIn("example_source: sha256:", output)

    def test_normative_paragraphs_ignore_fenced_code_and_headings(self) -> None:
        text = (
            "# MUST-like heading\n"
            "Ordinary prose.\n\n"
            "The implementation MUST preserve this.\n\n"
            "```text\n"
            "This apparent rule MUST be ignored.\n"
            "```\n"
            "A caller MAY select this.\n"
        )

        paragraphs = contracts.normative_paragraphs(text)

        self.assertEqual(
            [paragraph.text for paragraph in paragraphs],
            ["The implementation MUST preserve this.", "A caller MAY select this."],
        )

    def test_coverage_separates_draft_and_unchanged_reviewed_sources(self) -> None:
        document = self.write_document(
            "# Part\n"
            "intro\n"
            "## Subject\n"
            "The subject MUST be stable.\n"
            "### Nested\n"
            "The nested behavior SHOULD be deterministic.\n"
            "## Other\n"
            "The other behavior MAY vary.\n"
        )
        subject_source = self.source()
        subject_hash = contracts.section_hash(
            contracts.resolve_section(document, subject_source["heading_path"])
        )
        self.write_contract("reviewed.yaml", self.source(subject_hash), status="reviewed")
        self.write_contract(
            "draft.yaml",
            self.source(heading_path=["Part", "Other"]),
            status="draft",
        )

        report = contracts.coverage_report(
            documents=[document],
            contracts=contracts.contract_paths(),
        )

        result = report["documents"][0]
        self.assertEqual(result["totals"]["headings"], 4)
        self.assertEqual(result["totals"]["normative_paragraphs"], 3)
        self.assertEqual(result["coverage"]["referenced"]["headings"], 3)
        self.assertEqual(result["coverage"]["reviewed"]["headings"], 2)
        self.assertEqual(result["coverage"]["draft"]["headings"], 1)
        self.assertEqual(result["coverage"]["referenced"]["normative_paragraphs"], 3)
        self.assertEqual(result["coverage"]["reviewed"]["normative_paragraphs"], 2)
        self.assertEqual(result["coverage"]["draft"]["normative_paragraphs"], 1)

    def test_coverage_json_is_machine_readable(self) -> None:
        document = self.write_document("# Part\n## Subject\nThe value MUST be stable.\n")
        self.write_contract("example.yaml", self.source(), status="draft")

        _, output = self.capture(contracts.coverage, [document], "json")

        report = contracts.json.loads(output)
        self.assertEqual(report["documents"][0]["document"], "docs/specs/example.md")
        self.assertEqual(report["totals"]["coverage"]["draft"]["normative_paragraphs"], 1)

    def test_coverage_text_uses_percentage_tables(self) -> None:
        document = self.write_document("# Part\n## Subject\nThe value MUST be stable.\n")
        self.write_contract("example.yaml", self.source(), status="draft")

        _, output = self.capture(contracts.coverage, [document], "text")

        self.assertIn("Coverage percentages", output)
        self.assertIn("Referenced H/B/N (%)", output)
        self.assertIn("docs/specs/example.md", output)
        self.assertIn("50.0 /", output)
        self.assertIn("/ 100.0", output)
        self.assertIn("Citation diagnostics", output)

    def test_coverage_dump_retains_detailed_output(self) -> None:
        document = self.write_document("# Part\n## Subject\nThe value MUST be stable.\n")
        self.write_contract("example.yaml", self.source(), status="draft")

        _, output = self.capture(contracts.coverage, [document], "dump")

        self.assertIn("population: headings 2", output)
        self.assertIn("draft: headings 1 (50.0%)", output)
        self.assertIn("normative paragraphs 1 (100.0%)", output)


if __name__ == "__main__":
    unittest.main()
