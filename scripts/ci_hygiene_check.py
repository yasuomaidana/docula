#!/usr/bin/env python3
"""
CI Hygiene Gate:
1. Strict <= 500 lines per file (excluding tests and #[cfg(test)] modules).
2. Zero Duplicated Code Policy across workspace source files.
"""

import os
import re
import sys
from collections import defaultdict

MAX_LINES = 500
MIN_DUPE_LINES = 10

def is_source_dir(root: str) -> bool:
    norm = root.replace("\\", "/")
    if any(skip in norm for skip in ["/target", "/.git", "/.gemini", "/node_modules", "/tests", "/benches", "/examples"]):
        return False
    return norm.endswith("/src") or "/src/" in norm

def clean_tokens(line: str) -> str:
    line = re.sub(r'//.*$', '', line)
    return " ".join(line.split())

def check_file_line_limits(workspace_root: str):
    violations = []
    source_files = []

    for root, _, files in os.walk(workspace_root):
        if not is_source_dir(root):
            continue
        for f in files:
            if f.endswith(".rs"):
                full_path = os.path.join(root, f)
                source_files.append(full_path)
                with open(full_path, "r", encoding="utf-8", errors="ignore") as fp:
                    lines = fp.readlines()
                non_test = []
                for line in lines:
                    if line.strip().startswith("#[cfg(test)]"):
                        break
                    non_test.append(line)

                if len(non_test) > MAX_LINES:
                    violations.append((full_path, len(non_test)))

    return source_files, violations

def check_duplicated_code(source_files):
    block_occurrences = defaultdict(list)

    for path in source_files:
        with open(path, "r", encoding="utf-8", errors="ignore") as fp:
            raw_lines = fp.readlines()

        cleaned_lines = []
        for line_no, line in enumerate(raw_lines, 1):
            if line.strip().startswith("#[cfg(test)]"):
                break
            toks = clean_tokens(line)
            if toks and not toks.startswith(("//", "/*", "*", "///", "//!")):
                cleaned_lines.append((line_no, toks))

        # Sliding window of MIN_DUPE_LINES
        for i in range(len(cleaned_lines) - MIN_DUPE_LINES + 1):
            window = cleaned_lines[i : i + MIN_DUPE_LINES]
            # Ignore windows that are macro declarations, boilerplate closures or trivial braces
            content = tuple(tok for _, tok in window)
            is_macro_tokens = any(tok.startswith(("$", "macro_rules!")) for tok in content)
            if is_macro_tokens:
                continue

            braces_count = sum(1 for tok in content if tok in ["{", "}", "};", "});", "Ok(())", "Ok(())}", ")", "])"])
            if braces_count > (MIN_DUPE_LINES // 2):
                continue
            
            start_line = window[0][0]
            end_line = window[-1][0]
            block_occurrences[content].append((path, start_line, end_line))

    duplicates = {}
    for content, occurrences in block_occurrences.items():
        # Dedup occurrences within the exact same file and overlapping lines
        unique_locations = []
        seen_files = set()
        for path, start, end in occurrences:
            if path not in seen_files:
                unique_locations.append((path, start, end))
                seen_files.add(path)

        if len(unique_locations) > 1:
            duplicates[content] = unique_locations

    return duplicates

def main():
    workspace = sys.argv[1] if len(sys.argv) > 1 else "."
    print("=" * 70)
    print("Running CI Hygiene Gates: 500-Line Limit & Zero Duplicated Code")
    print("=" * 70)

    source_files, line_violations = check_file_line_limits(workspace)
    print(f"[✓] Audited {len(source_files)} Rust source files for line limits.")

    failed = False
    if line_violations:
        print(f"\n[FAIL] Found {len(line_violations)} files exceeding {MAX_LINES} non-test lines:")
        for path, count in line_violations:
            print(f"  - {path}: {count} lines")
        failed = True
    else:
        print(f"[PASS] All {len(source_files)} source files are within the <= {MAX_LINES} non-test lines limit.")

    print("\n[i] Scanning for code block duplication across workspace crates...")
    dupes = check_duplicated_code(source_files)

    if dupes:
        print(f"\n[FAIL] Found {len(dupes)} duplicated code blocks across different files:")
        for idx, (content, locs) in enumerate(dupes.items(), 1):
            print(f"\n--- Duplication #{idx} ({MIN_DUPE_LINES} normalized lines) ---")
            for path, s, e in locs:
                print(f"  at {path}:{s}-{e}")
            snippet = "\n    ".join(content[:3])
            print(f"  preview:\n    {snippet}\n    ...")
        failed = True
    else:
        print("[PASS] Zero duplicated code blocks detected across workspace crates.")

    print("\n" + "=" * 70)
    if failed:
        print("CI HYGIENE AUDIT FAILED")
        print("=" * 70)
        sys.exit(1)
    else:
        print("CI HYGIENE AUDIT PASSED")
        print("=" * 70)
        sys.exit(0)

if __name__ == "__main__":
    main()
