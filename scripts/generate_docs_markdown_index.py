#!/usr/bin/env python3
import argparse
import collections
import os
import re
import sys

workspace_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
docs_dir = os.path.join(workspace_root, "docs")

FENCE_PATTERN = re.compile(r"^[ ]{0,3}(`{3,}|~{3,})")
CLOSING_FENCE_PATTERN = re.compile(r"^[ ]{0,3}(`+|~+)[ \t]*$")
HEADING_PATTERN = re.compile(r"^[ ]{0,3}(#{1,6})(?:[ \t]+(.*))?$")
BACKTICK_WORD_PATTERN = re.compile(r"`([^`\n]+)`")
WHITESPACE_PATTERN = re.compile(r"\s+")
RULE_PATTERN = re.compile(r"^[ ]{0,3}([-*_][ \t]*){3,}$")
NORMATIVE_WORD_PATTERN = re.compile(r"\b(MUST NOT|SHOULD NOT|MUST|SHOULD|MAY)\b")

COMMON_SYMBOLS = {
    "'static",
    "Add",
    "All",
    "Any",
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "D",
    "Eq",
    "Hash",
    "I",
    "Into",
    "IntoIterator",
    "Iterator",
    "O",
    "Option",
    "Ord",
    "PartialEq",
    "PartialOrd",
    "PhantomData",
    "Result",
    "S",
    "Self",
    "Send",
    "Sized",
    "Sub",
    "Sync",
    "T",
    "TryFrom",
    "TryInto",
    "Vec",
    "bool",
    "i32",
    "i64",
    "impl Into<String>",
    "str",
    "u16",
    "u32",
    "u64",
    "u8",
    "usize",
}


def parse_markdown_line(line, in_code_block, fence_char, fence_len):
    if in_code_block:
        closing_match = CLOSING_FENCE_PATTERN.match(line)
        if closing_match:
            fence = closing_match.group(1)
            if fence[0] == fence_char and len(fence) >= fence_len:
                return False, None, 0, None
        return True, fence_char, fence_len, None

    opening_match = FENCE_PATTERN.match(line)
    if opening_match:
        fence = opening_match.group(1)
        return True, fence[0], len(fence), None

    heading_match = HEADING_PATTERN.match(line)
    if not heading_match:
        return False, None, 0, None

    level = len(heading_match.group(1))
    raw_title = heading_match.group(2) if heading_match.group(2) else ""
    title = re.sub(r"[ \t]+#+[ \t]*$", "", raw_title).strip()
    return False, None, 0, {"level": level, "title": title}


def parse_headings(content):
    lines = content.splitlines()
    total_line_count = len(lines)
    headings = []
    in_code_block = False
    fence_char = None
    fence_len = 0

    for idx, line in enumerate(lines):
        line_num = idx + 1
        in_code_block, fence_char, fence_len, heading = parse_markdown_line(
            line,
            in_code_block,
            fence_char,
            fence_len,
        )
        if heading is not None:
            headings.append({
                "level": heading["level"],
                "title": heading["title"],
                "start_line": line_num,
                "end_line": line_num,
            })

    for i in range(len(headings)):
        end_line = total_line_count
        for j in range(i + 1, len(headings)):
            if headings[j]["level"] <= headings[i]["level"]:
                end_line = headings[j]["start_line"] - 1
                break
        headings[i]["end_line"] = end_line

    return headings


def build_heading_paths(headings):
    path_by_start_line = {}
    stack = []
    for heading in headings:
        while stack and stack[-1]["level"] >= heading["level"]:
            stack.pop()
        stack.append(heading)
        path_by_start_line[heading["start_line"]] = " > ".join(
            entry["title"] for entry in stack
        )
    return path_by_start_line


def section_text(lines, heading):
    start_idx = heading["start_line"]
    end_idx = heading["end_line"]
    return "\n".join(lines[start_idx:end_idx])


def extract_first_sentence(text):
    normalized = WHITESPACE_PATTERN.sub(" ", text).strip()
    if not normalized:
        return ""

    sentence_match = re.match(r"(.+?[.!?](?=\s|$))", normalized)
    if sentence_match:
        return sentence_match.group(1).strip()
    return normalized


def extract_backtick_words(text):
    counts = collections.Counter()
    first_seen = {}
    for index, match in enumerate(BACKTICK_WORD_PATTERN.finditer(text)):
        symbol = match.group(1).strip()
        if not symbol or symbol in COMMON_SYMBOLS or re.fullmatch(r"[A-Z]", symbol):
            continue
        counts[symbol] += 1
        first_seen.setdefault(symbol, index)

    return [
        symbol for symbol, _ in sorted(
            counts.items(),
            key=lambda item: (-item[1], first_seen[item[0]], item[0]),
        )
    ]


def extract_normative_terms(text):
    counts = collections.Counter()
    for match in NORMATIVE_WORD_PATTERN.finditer(text):
        counts[match.group(1)] += 1

    ordered_terms = ["MUST NOT", "MUST", "SHOULD NOT", "SHOULD", "MAY"]
    return [f"{term} {counts[term]}" for term in ordered_terms if counts.get(term, 0)]


def extract_preview_and_symbols(section_content):
    lines = section_content.splitlines()
    kept_lines = []
    in_code_block = False
    fence_char = None
    fence_len = 0

    for line in lines:
        in_code_block, fence_char, fence_len, _ = parse_markdown_line(
            line,
            in_code_block,
            fence_char,
            fence_len,
        )
        if in_code_block or FENCE_PATTERN.match(line):
            continue
        if HEADING_PATTERN.match(line) or RULE_PATTERN.match(line):
            continue
        kept_lines.append(line)

    text = "\n".join(kept_lines)
    return (
        extract_first_sentence(text),
        extract_backtick_words(text),
        extract_normative_terms(text),
    )


def load_doc(abs_path):
    with open(abs_path, "r", encoding="utf-8") as f:
        content = f.read()

    return {
        "abs_path": abs_path,
        "rel_path": os.path.relpath(abs_path, docs_dir),
        "content": content,
        "headings": parse_headings(content),
    }


def generate_markdown_index(doc):
    lines = doc["content"].splitlines()
    heading_paths = build_heading_paths(doc["headings"])
    output = [f"## docs/{doc['rel_path'].replace(os.sep, '/')}"]

    for heading in doc["headings"]:
        preview, symbols, normative = extract_preview_and_symbols(section_text(lines, heading))
        output.append(
            f"- `{heading_paths[heading['start_line']]}` "
            f"[{heading['start_line']}-{heading['end_line']}]"
        )
        if preview:
            output.append(f"  Preview: {preview}")
        if symbols:
            output.append("  Symbols: " + ", ".join(f"`{symbol}`" for symbol in symbols))
        if normative:
            output.append("  Normative: " + ", ".join(normative))
        output.append("")

    return "\n".join(output).rstrip() + "\n"


def parse_args(argv):
    parser = argparse.ArgumentParser()
    parser.add_argument("doc")
    parser.add_argument("--output")
    return parser.parse_args(argv)


def main():
    args = parse_args(sys.argv[1:])
    abs_path = os.path.abspath(args.doc)
    if not abs_path.startswith(docs_dir + os.sep):
        print("Error: document must point inside docs/.", file=sys.stderr)
        sys.exit(1)

    try:
        doc = load_doc(abs_path)
    except Exception as e:
        print(f"Error loading document: {e}", file=sys.stderr)
        sys.exit(1)

    generated_markdown = generate_markdown_index(doc)
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(generated_markdown)
    else:
        sys.stdout.write(generated_markdown)


if __name__ == "__main__":
    main()
