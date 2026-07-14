#!/usr/bin/env python3
"""Inspect specification-contract source evidence without modifying contracts."""

from __future__ import annotations

import argparse
import hashlib
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


class ContractError(ValueError):
    """A selected contract cannot be inspected deterministically."""


@dataclass(frozen=True)
class Heading:
    level: int
    text: str
    line: int
    path: tuple[str, ...]


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


def parse_headings(text: str) -> list[Heading]:
    stack: list[tuple[int, str]] = []
    headings: list[Heading] = []
    for line_number, line in enumerate(text.splitlines(), start=1):
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


def resolve_section(document: Path, heading_path: object) -> str:
    if not isinstance(heading_path, list) or not all(
        isinstance(part, str) and part for part in heading_path
    ):
        raise ContractError("heading_path must be a non-empty sequence of heading text")
    try:
        text = document.read_text(encoding="utf-8")
    except OSError as error:
        raise ContractError(f"missing document {document}: {error}") from error

    requested_path = tuple(heading_path)
    matches = [heading for heading in parse_headings(text) if heading.path == requested_path]
    if not matches:
        raise LookupError("missing")
    if len(matches) != 1:
        raise LookupError("ambiguous")

    heading = matches[0]
    lines = text.splitlines(keepends=True)
    end = len(lines)
    for candidate in parse_headings(text):
        if candidate.line > heading.line and candidate.level <= heading.level:
            end = candidate.line - 1
            break
    return "".join(lines[heading.line - 1 : end])


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
    for command in ("status", "fingerprint"):
        subparser = subcommands.add_parser(command)
        subparser.add_argument("contracts", nargs="+", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "catalog":
            return catalog()
        if args.command == "status":
            return status(args.contracts)
        return fingerprint(args.contracts)
    except ContractError as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
