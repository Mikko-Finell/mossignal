#!/usr/bin/env python3
"""Inspect specification-contract source evidence without modifying contracts."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:  # pragma: no cover - exercised when tooling is misconfigured.
    yaml = None


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
CONTRACTS_DIRECTORY = REPOSITORY_ROOT / "docs/specs/contracts"
HEADING_PATTERN = re.compile(r"^(#{1,6})[ \t]+(.*?)(?:[ \t]+#+[ \t]*)?$")
FENCE_OPENING_PATTERN = re.compile(r"^[ ]{0,3}(`{3,}|~{3,})")
NORMATIVE_PATTERN = re.compile(r"\b(?:MUST NOT|SHOULD NOT|MUST|SHOULD|MAY)\b")


class ContractError(ValueError):
    """A selected contract cannot be inspected deterministically."""


@dataclass(frozen=True)
class Heading:
    level: int
    text: str
    line: int
    path: tuple[str, ...]


@dataclass(frozen=True)
class Paragraph:
    start_line: int
    end_line: int
    text: str


def require_yaml() -> Any:
    if yaml is None:
        raise ContractError(
            "PyYAML is required for scripts/contracts.py. "
            "Install the repository's agent-tool dependencies."
        )
    return yaml


def load_contract(path: Path) -> dict[str, Any]:
    try:
        document = require_yaml().safe_load(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise ContractError(f"cannot read {path}: {error}") from error
    except Exception as error:
        raise ContractError(f"cannot parse YAML in {path}: {error}") from error
    if not isinstance(document, dict):
        raise ContractError(f"{path} must contain a YAML mapping")
    return document


def normalize_section(text: str) -> bytes:
    """Apply the contract format's formatting-only fingerprint normalization."""
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    normalized = "\n".join(line.rstrip(" \t") for line in normalized.split("\n"))
    return (normalized.rstrip("\n") + "\n").encode("utf-8")


def section_hash(text: str) -> str:
    return "sha256:" + hashlib.sha256(normalize_section(text)).hexdigest()


def opening_fence(line: str) -> tuple[str, int] | None:
    match = FENCE_OPENING_PATTERN.match(line)
    if match is None:
        return None
    marker = match.group(1)
    return marker[0], len(marker)


def closes_fence(line: str, marker: str, minimum_length: int) -> bool:
    return re.match(
        rf"^[ ]{{0,3}}{re.escape(marker)}{{{minimum_length},}}[ \t]*$",
        line,
    ) is not None


def parse_headings(text: str) -> list[Heading]:
    stack: list[tuple[int, str]] = []
    headings: list[Heading] = []
    fence: tuple[str, int] | None = None
    for line_number, line in enumerate(text.splitlines(), start=1):
        if fence is not None:
            if closes_fence(line, *fence):
                fence = None
            continue
        fence = opening_fence(line)
        if fence is not None:
            continue
        match = HEADING_PATTERN.match(line)
        if match is None:
            continue
        level = len(match.group(1))
        heading_text = match.group(2).rstrip(" \t")
        while stack and stack[-1][0] >= level:
            stack.pop()
        stack.append((level, heading_text))
        headings.append(
            Heading(
                level=level,
                text=heading_text,
                line=line_number,
                path=tuple(item[1] for item in stack),
            )
        )
    return headings


def resolve_section_bounds(text: str, heading_path: object) -> tuple[int, int]:
    if not isinstance(heading_path, list) or not all(
        isinstance(part, str) and part for part in heading_path
    ):
        raise ContractError("heading_path must be a non-empty sequence of heading text")

    requested_path = tuple(heading_path)
    headings = parse_headings(text)
    matches = [heading for heading in headings if heading.path == requested_path]
    if not matches:
        raise LookupError("missing")
    if len(matches) != 1:
        raise LookupError("ambiguous")

    heading = matches[0]
    lines = text.splitlines(keepends=True)
    end = len(lines)
    for candidate in headings:
        if candidate.line > heading.line and candidate.level <= heading.level:
            end = candidate.line - 1
            break
    return heading.line - 1, end


def resolve_section(document: Path, heading_path: object) -> str:
    try:
        text = document.read_text(encoding="utf-8")
    except OSError as error:
        raise ContractError(f"missing document {document}: {error}") from error
    start, end = resolve_section_bounds(text, heading_path)
    return "".join(text.splitlines(keepends=True)[start:end])


def markdown_paragraphs(text: str) -> list[Paragraph]:
    """Return prose paragraphs with fenced code excluded."""
    paragraphs: list[Paragraph] = []
    paragraph_lines: list[str] = []
    paragraph_start = 0
    fence: tuple[str, int] | None = None

    def finish(end_line: int) -> None:
        nonlocal paragraph_lines, paragraph_start
        if paragraph_lines:
            paragraphs.append(
                Paragraph(
                    start_line=paragraph_start,
                    end_line=end_line,
                    text="\n".join(paragraph_lines),
                )
            )
            paragraph_lines = []

    for line_number, line in enumerate(text.splitlines(), start=1):
        if fence is not None:
            if closes_fence(line, *fence):
                fence = None
            continue
        opened_fence = opening_fence(line)
        if opened_fence is not None:
            finish(line_number - 1)
            fence = opened_fence
            continue
        if HEADING_PATTERN.match(line) is not None:
            finish(line_number - 1)
            continue
        if not line.strip():
            finish(line_number - 1)
            continue
        if not paragraph_lines:
            paragraph_start = line_number
        paragraph_lines.append(line)
    finish(len(text.splitlines()))
    return paragraphs


def normative_paragraphs(text: str) -> list[Paragraph]:
    return [paragraph for paragraph in markdown_paragraphs(text) if NORMATIVE_PATTERN.search(paragraph.text)]


def default_specification_documents() -> list[Path]:
    documents = sorted((REPOSITORY_ROOT / "docs/specs").glob("*.md"))
    testing_policy = REPOSITORY_ROOT / "docs/testing_and_verification_policy.md"
    if testing_policy.exists():
        documents.append(testing_policy)
    return documents


def contract_paths() -> list[Path]:
    return [
        path
        for path in sorted(CONTRACTS_DIRECTORY.glob("*.yaml"))
        if path.name != "_template.yaml"
    ]


def line_bytes(text: str) -> list[int]:
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    return [len((line.rstrip(" \t") + "\n").encode("utf-8")) for line in normalized.splitlines()]


def percentage(part: int, whole: int) -> float:
    return round(100.0 * part / whole, 1) if whole else 0.0


def coverage_counts(
    text: str,
    line_numbers: set[int],
) -> dict[str, int | float]:
    headings = parse_headings(text)
    normative = normative_paragraphs(text)
    byte_weights = line_bytes(text)
    covered_bytes = sum(byte_weights[line - 1] for line in line_numbers)
    covered_headings = sum(heading.line in line_numbers for heading in headings)
    covered_normative = sum(
        any(line in line_numbers for line in range(paragraph.start_line, paragraph.end_line + 1))
        for paragraph in normative
    )
    return {
        "headings": covered_headings,
        "heading_percent": percentage(covered_headings, len(headings)),
        "bytes": covered_bytes,
        "byte_percent": percentage(covered_bytes, sum(byte_weights)),
        "normative_paragraphs": covered_normative,
        "normative_percent": percentage(covered_normative, len(normative)),
    }


def coverage_report(
    documents: list[Path] | None = None,
    contracts: list[Path] | None = None,
) -> dict[str, Any]:
    selected_documents = documents if documents is not None else default_specification_documents()
    selected_contracts = contracts if contracts is not None else contract_paths()
    document_text: dict[Path, str] = {}
    for document in selected_documents:
        resolved = document if document.is_absolute() else REPOSITORY_ROOT / document
        try:
            document_text[resolved.resolve()] = resolved.read_text(encoding="utf-8")
        except OSError as error:
            raise ContractError(f"cannot read {resolved}: {error}") from error

    coverage_lines: dict[Path, dict[str, set[int]]] = {
        document: {"referenced": set(), "draft": set(), "reviewed": set()}
        for document in document_text
    }
    citation_counts = {document: 0 for document in document_text}
    overlapping_citations = {document: 0 for document in document_text}
    broad_citations = {document: 0 for document in document_text}
    outside_corpus = 0
    unused_sources = 0
    unresolved_sources = 0
    contract_status_counts: dict[str, int] = {}

    for contract_path in selected_contracts:
        contract = load_contract(contract_path)
        contract_status = contract.get("status")
        if not isinstance(contract_status, str):
            raise ContractError(f"{contract_path} must declare a string status")
        contract_status_counts[contract_status] = contract_status_counts.get(contract_status, 0) + 1
        sources = contract.get("sources", {})
        if not isinstance(sources, dict):
            raise ContractError(f"{contract_path} sources must be a mapping")
        for alias, source in sources.items():
            if not isinstance(alias, str) or not isinstance(source, dict):
                raise ContractError(f"{contract_path} sources must map string aliases to mappings")
            if not source_rules(contract, alias):
                unused_sources += 1
                continue
            source_document = source.get("document")
            if not isinstance(source_document, str) or not source_document:
                raise ContractError(f"source {alias} must declare a document")
            source_path = (REPOSITORY_ROOT / source_document).resolve()
            if source_path not in document_text:
                outside_corpus += 1
                continue
            text = document_text[source_path]
            try:
                start, end = resolve_section_bounds(text, source.get("heading_path"))
            except LookupError:
                unresolved_sources += 1
                continue
            lines = set(range(start + 1, end + 1))
            if coverage_lines[source_path]["referenced"].intersection(lines):
                overlapping_citations[source_path] += 1
            if len(lines) > 200 or len(lines) * 5 > max(1, len(text.splitlines())):
                broad_citations[source_path] += 1
            citation_counts[source_path] += 1
            coverage_lines[source_path]["referenced"].update(lines)
            if contract_status == "draft":
                coverage_lines[source_path]["draft"].update(lines)
            source_status, _ = inspect_source(contract, alias, source)
            if contract_status == "reviewed" and source_status == "unchanged":
                coverage_lines[source_path]["reviewed"].update(lines)

    document_results: list[dict[str, Any]] = []
    total_headings = 0
    total_bytes = 0
    total_normative = 0
    total_counts = {
        category: {"headings": 0, "bytes": 0, "normative_paragraphs": 0}
        for category in ("referenced", "draft", "reviewed")
    }

    for document, text in document_text.items():
        headings = parse_headings(text)
        normative = normative_paragraphs(text)
        byte_weights = line_bytes(text)
        categories = {
            category: coverage_counts(text, lines)
            for category, lines in coverage_lines[document].items()
        }
        for category, counts in categories.items():
            for key in total_counts[category]:
                total_counts[category][key] += int(counts[key])
        total_headings += len(headings)
        total_bytes += sum(byte_weights)
        total_normative += len(normative)
        try:
            display_path = str(document.relative_to(REPOSITORY_ROOT.resolve()))
        except ValueError:
            display_path = str(document)
        document_results.append(
            {
                "document": display_path,
                "totals": {
                    "headings": len(headings),
                    "bytes": sum(byte_weights),
                    "normative_paragraphs": len(normative),
                },
                "coverage": categories,
                "citations": citation_counts[document],
                "overlapping_citations": overlapping_citations[document],
                "broad_citations": broad_citations[document],
            }
        )

    for category, counts in total_counts.items():
        counts["heading_percent"] = percentage(counts["headings"], total_headings)
        counts["byte_percent"] = percentage(counts["bytes"], total_bytes)
        counts["normative_percent"] = percentage(counts["normative_paragraphs"], total_normative)

    return {
        "warning": "Heuristic evidence footprint; citations do not prove semantic completeness.",
        "contracts": contract_status_counts,
        "documents": document_results,
        "totals": {
            "population": {
                "headings": total_headings,
                "bytes": total_bytes,
                "normative_paragraphs": total_normative,
            },
            "coverage": total_counts,
        },
        "references_outside_selected_corpus": outside_corpus,
        "unused_contract_sources": unused_sources,
        "unresolved_contract_sources": unresolved_sources,
    }


def percentage_triplet(counts: dict[str, Any]) -> str:
    return (
        f"{counts['heading_percent']:.1f} / "
        f"{counts['byte_percent']:.1f} / "
        f"{counts['normative_percent']:.1f}"
    )


def print_table(
    headers: list[str],
    rows: list[list[str]],
    right_aligned: set[int] | None = None,
) -> None:
    align_right = right_aligned or set()
    widths = [
        max(len(header), *(len(row[index]) for row in rows))
        for index, header in enumerate(headers)
    ]

    def render(row: list[str]) -> str:
        cells = [
            value.rjust(widths[index]) if index in align_right else value.ljust(widths[index])
            for index, value in enumerate(row)
        ]
        return "  ".join(cells).rstrip()

    print(render(headers))
    print("  ".join("-" * width for width in widths))
    for row in rows:
        print(render(row))


def format_coverage_counts(label: str, counts: dict[str, Any]) -> str:
    return (
        f"  {label}: headings {counts['headings']} ({counts['heading_percent']:.1f}%), "
        f"bytes {counts['bytes']} ({counts['byte_percent']:.1f}%), "
        f"normative paragraphs {counts['normative_paragraphs']} "
        f"({counts['normative_percent']:.1f}%)"
    )


def print_coverage_dump(report: dict[str, Any]) -> None:
    for document in report["documents"]:
        print(document["document"])
        totals = document["totals"]
        print(
            f"  population: headings {totals['headings']}, bytes {totals['bytes']}, "
            f"normative paragraphs {totals['normative_paragraphs']}"
        )
        for category in ("referenced", "reviewed", "draft"):
            print(format_coverage_counts(category, document["coverage"][category]))
        print(
            f"  citations: {document['citations']}; overlaps: "
            f"{document['overlapping_citations']}; broad: {document['broad_citations']}"
        )
    print("TOTAL")
    population = report["totals"]["population"]
    print(
        f"  population: headings {population['headings']}, bytes {population['bytes']}, "
        f"normative paragraphs {population['normative_paragraphs']}"
    )
    for category in ("referenced", "reviewed", "draft"):
        print(format_coverage_counts(category, report["totals"]["coverage"][category]))
    print(f"references outside selected corpus: {report['references_outside_selected_corpus']}")
    print(f"unused contract sources: {report['unused_contract_sources']}")
    print(f"unresolved contract sources: {report['unresolved_contract_sources']}")


def coverage(documents: list[Path] | None, output_format: str) -> int:
    report = coverage_report(documents=documents)
    if output_format == "json":
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0

    print("Contract evidence coverage")
    print(f"WARNING: {report['warning']}")
    contract_summary = ", ".join(
        f"{status}={count}" for status, count in sorted(report["contracts"].items())
    ) or "none"
    print(f"Contracts: {contract_summary}")
    if output_format == "dump":
        print_coverage_dump(report)
        return 0
    print()
    print("Coverage percentages")
    print("H / B / N = headings / normalized bytes / normative paragraphs")
    coverage_rows = []
    for document in report["documents"]:
        coverage_rows.append(
            [
                document["document"],
                percentage_triplet(document["coverage"]["referenced"]),
                percentage_triplet(document["coverage"]["reviewed"]),
                percentage_triplet(document["coverage"]["draft"]),
            ]
        )
    coverage_rows.append(
        [
            "TOTAL",
            percentage_triplet(report["totals"]["coverage"]["referenced"]),
            percentage_triplet(report["totals"]["coverage"]["reviewed"]),
            percentage_triplet(report["totals"]["coverage"]["draft"]),
        ]
    )
    print_table(
        ["Document", "Referenced H/B/N (%)", "Reviewed H/B/N (%)", "Draft H/B/N (%)"],
        coverage_rows,
        right_aligned={1, 2, 3},
    )

    print()
    print("Corpus size")
    population = report["totals"]["population"]
    corpus_rows = [
        [
            document["document"],
            str(document["totals"]["headings"]),
            str(document["totals"]["bytes"]),
            str(document["totals"]["normative_paragraphs"]),
        ]
        for document in report["documents"]
    ]
    corpus_rows.append(
        [
            "TOTAL",
            str(population["headings"]),
            str(population["bytes"]),
            str(population["normative_paragraphs"]),
        ]
    )
    print_table(
        ["Document", "Headings", "Normalized bytes", "Normative paragraphs"],
        corpus_rows,
        right_aligned={1, 2, 3},
    )

    print()
    print("Citation diagnostics")
    diagnostic_rows = [
        [
            document["document"],
            str(document["citations"]),
            str(document["overlapping_citations"]),
            str(document["broad_citations"]),
        ]
        for document in report["documents"]
        if document["citations"]
        or document["overlapping_citations"]
        or document["broad_citations"]
    ]
    if diagnostic_rows:
        print_table(
            ["Document", "Citations", "Overlaps", "Broad"],
            diagnostic_rows,
            right_aligned={1, 2, 3},
        )
    else:
        print("No citations in the selected corpus.")
    print(
        "Outside selected corpus: "
        f"{report['references_outside_selected_corpus']}  |  "
        f"Unused sources: {report['unused_contract_sources']}  |  "
        f"Unresolved sources: {report['unresolved_contract_sources']}"
    )
    return 0


def source_rules(contract: dict[str, Any], source_alias: str) -> list[str]:
    result: list[str] = []
    for collection_name in ("requirements", "recommendations"):
        collection = contract.get(collection_name, [])
        if not isinstance(collection, list):
            continue
        for rule in collection:
            if not isinstance(rule, dict):
                continue
            sources = rule.get("sources", [])
            rule_id = rule.get("id")
            if source_alias in sources and isinstance(rule_id, str) and rule_id not in result:
                result.append(rule_id)
    return result


def inspect_source(contract: dict[str, Any], alias: str, source: object) -> tuple[str, str | None]:
    if not isinstance(source, dict):
        raise ContractError(f"source {alias} must be a mapping")
    document = source.get("document")
    if not isinstance(document, str) or not document:
        raise ContractError(f"source {alias} must declare a document")
    document_path = REPOSITORY_ROOT / document
    try:
        current_hash = section_hash(resolve_section(document_path, source.get("heading_path")))
    except LookupError as error:
        return str(error), None
    except ContractError as error:
        if str(error).startswith("missing document "):
            return "missing", None
        raise

    reviewed_hash = source.get("reviewed_hash")
    if reviewed_hash is None:
        return "not_fingerprinted", current_hash
    if not isinstance(reviewed_hash, str) or not reviewed_hash.startswith("sha256:"):
        raise ContractError(f"source {alias} has an invalid reviewed_hash")
    return ("unchanged" if reviewed_hash == current_hash else "changed"), current_hash


def catalog() -> int:
    for path in sorted(CONTRACTS_DIRECTORY.glob("*.yaml")):
        if path.name == "_template.yaml":
            continue
        contract = load_contract(path)
        contract_id = contract.get("id", "<missing id>")
        status = contract.get("status", "<missing status>")
        title = contract.get("title", "<missing title>")
        summary = contract.get("summary", "<missing summary>")
        aliases = contract.get("aliases", [])
        alias_text = ", ".join(aliases) if isinstance(aliases, list) else "<invalid aliases>"
        print(contract_id)
        print(f"  status: {status}")
        print(f"  title: {title}")
        print(f"  summary: {summary}")
        print(f"  aliases: {alias_text}")
    return 0


def status(paths: list[Path]) -> int:
    for path in paths:
        contract = load_contract(path)
        print(path)
        sources = contract.get("sources", {})
        if not isinstance(sources, dict):
            raise ContractError(f"{path} sources must be a mapping")
        for alias, source in sources.items():
            if not isinstance(alias, str):
                raise ContractError(f"{path} source aliases must be strings")
            source_status, _ = inspect_source(contract, alias, source)
            print(f"  {alias}")
            print(f"    status: {source_status}")
            print("    affected_rules:")
            for rule_id in source_rules(contract, alias):
                print(f"      - {rule_id}")
    return 0


def fingerprint(paths: list[Path]) -> int:
    output: dict[str, str] = {}
    for path in paths:
        contract = load_contract(path)
        sources = contract.get("sources", {})
        if not isinstance(sources, dict):
            raise ContractError(f"{path} sources must be a mapping")
        for alias, source in sources.items():
            if not isinstance(alias, str):
                raise ContractError(f"{path} source aliases must be strings")
            _, current_hash = inspect_source(contract, alias, source)
            if current_hash is not None:
                output[alias] = current_hash
    print(require_yaml().safe_dump(output, sort_keys=False).rstrip())
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("catalog")
    coverage_parser = subcommands.add_parser("coverage")
    coverage_parser.add_argument(
        "--document",
        action="append",
        type=Path,
        dest="documents",
        help="limit coverage to one specification document; repeatable",
    )
    coverage_parser.add_argument("--format", choices=("text", "json", "dump"), default="text")
    for command in ("status", "fingerprint"):
        subparser = subcommands.add_parser(command)
        subparser.add_argument("contracts", nargs="+", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "catalog":
            return catalog()
        if args.command == "coverage":
            return coverage(args.documents, args.format)
        if args.command == "status":
            return status(args.contracts)
        return fingerprint(args.contracts)
    except ContractError as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
