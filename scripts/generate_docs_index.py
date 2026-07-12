#!/usr/bin/env python3
import os
import sys
import re
import json

workspace_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
docs_dir = os.path.join(workspace_root, "docs")
index_json_path = os.path.join(docs_dir, "index.json")

def parse_metadata(content, filepath):
    # Find H1
    lines = content.splitlines()
    h1_idx = -1
    for idx, line in enumerate(lines):
        if line.startswith("# "):
            h1_idx = idx
            break
    if h1_idx == -1:
        raise ValueError(f"No H1 title found in {filepath}")

    title = lines[h1_idx][2:].strip()

    defines = None
    
    # Check the document metadata immediately following the H1.
    for idx in range(h1_idx + 1, min(h1_idx + 15, len(lines))):
        line = lines[idx].strip()
        if line.startswith("**Defines:**"):
            if defines is not None:
                raise ValueError(f"Duplicate '**Defines:**' found in {filepath}")
            defines = line[len("**Defines:**"):].strip()

    if not defines:
        raise ValueError(f"Missing or empty '**Defines:**' metadata after H1 in {filepath}")

    return title, defines

def parse_headings(content):
    """
    Statefully scans Markdown content for ATX headings (1 to 6 '#' characters).
    Ignores headings inside fenced code blocks using backticks (`) or tildes (~).
    Computes inclusive start_line and end_line for each heading.
    """
    lines = content.splitlines()
    total_line_count = len(lines)
    headings = []
    
    in_code_block = False
    fence_char = None
    fence_len = 0
    
    for idx, line in enumerate(lines):
        line_num = idx + 1
        
        # Check code blocks
        if in_code_block:
            # Look for closing fence
            closing_match = re.match(r"^[ ]{0,3}(`+|~+)[ \t]*$", line)
            if closing_match:
                fence = closing_match.group(1)
                if fence[0] == fence_char and len(fence) >= fence_len:
                    in_code_block = False
                    fence_char = None
                    fence_len = 0
            continue
        else:
            # Look for opening fence
            opening_match = re.match(r"^[ ]{0,3}(`{3,}|~{3,})", line)
            if opening_match:
                fence = opening_match.group(1)
                in_code_block = True
                fence_char = fence[0]
                fence_len = len(fence)
                continue
            
            # Look for ATX heading
            heading_match = re.match(r"^[ ]{0,3}(#{1,6})(?:[ \t]+(.*))?$", line)
            if heading_match:
                level = len(heading_match.group(1))
                raw_title = heading_match.group(2) if heading_match.group(2) else ""
                # Strip trailing hashes preceded by whitespace, per GFM ATX spec
                title = re.sub(r"[ \t]+#+[ \t]*$", "", raw_title).strip()
                headings.append({
                    "level": level,
                    "title": title,
                    "start_line": line_num,
                    "end_line": line_num # temporary placeholder
                })
                
    # Compute inclusive end_line for each heading
    for i in range(len(headings)):
        end_line = total_line_count
        for j in range(i + 1, len(headings)):
            if headings[j]["level"] <= headings[i]["level"]:
                end_line = headings[j]["start_line"] - 1
                break
        headings[i]["end_line"] = end_line
        
    return headings

def get_indexed_docs():
    indexed_docs = []
    for root, _, files in os.walk(docs_dir):
        for file in files:
            if file.endswith(".md"):
                abs_path = os.path.join(root, file)
                rel_path = os.path.relpath(abs_path, docs_dir)
                indexed_docs.append((abs_path, rel_path))
    indexed_docs.sort(key=lambda x: x[1])
    return indexed_docs

def generate_index_json(docs_metadata):
    documents = []
    for doc in docs_metadata:
        repo_path = "docs/" + doc["rel_path"].replace(os.sep, "/")
        documents.append({
            "path": repo_path,
            "title": doc["title"],
            "defines": doc["defines"],
            "line_count": doc["line_count"],
            "headings": doc["headings"]
        })
    # Keep paths consistently sorted
    documents.sort(key=lambda x: x["path"])
    
    manifest = {
        "schema_version": "1.0",
        "documents": documents
    }
    return json.dumps(manifest, indent=2, sort_keys=True)

def main():
    check_mode = "--check" in sys.argv
    
    try:
        indexed_docs = get_indexed_docs()
    except Exception as e:
        print(f"Error gathering indexed docs: {e}", file=sys.stderr)
        sys.exit(1)

    docs_metadata = []
    errors = []

    for abs_path, rel_path in indexed_docs:
        try:
            with open(abs_path, "r", encoding="utf-8") as f:
                content = f.read()
            title, defines = parse_metadata(content, rel_path)
            headings = parse_headings(content)
            line_count = len(content.splitlines())
            
            docs_metadata.append({
                "abs_path": abs_path,
                "rel_path": rel_path,
                "title": title,
                "defines": defines,
                "line_count": line_count,
                "headings": headings
            })
        except Exception as e:
            errors.append(str(e))

    if errors:
        print("Metadata validation errors found:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        sys.exit(1)

    # Generate content
    generated_json = generate_index_json(docs_metadata)

    if check_mode:
        if not os.path.exists(index_json_path):
            print("Error: docs/index.json is missing.", file=sys.stderr)
            sys.exit(1)
        with open(index_json_path, "r", encoding="utf-8") as f:
            existing_json = f.read()
        if existing_json != generated_json:
            print("Error: docs/index.json is stale. Run scripts/generate_docs_index.py to regenerate it.", file=sys.stderr)
            sys.exit(1)

        print("Check passed: docs/index.json is up-to-date and all metadata is valid.")
        sys.exit(0)
    else:
        with open(index_json_path, "w", encoding="utf-8") as f:
            f.write(generated_json)
        print("docs/index.json successfully updated.")

if __name__ == "__main__":
    main()
