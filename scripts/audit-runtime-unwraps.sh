#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/audit-runtime-unwraps.sh [--report|--enforce]

Reports the runtime unwrap governance state for ntgw-ai production sources.
`--enforce` fails if any governed production source still contains unwrap/expect.
USAGE
}

mode="report"
case "${1:-}" in
  ""|--report)
    ;;
  --enforce)
    mode="enforce"
    ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "$repo_root"

audit_doc="docs/runtime-unwrap-ntgw-ai-zero-tolerance.md"
if [[ ! -f "$audit_doc" ]]; then
  echo "::error::${audit_doc} is missing" >&2
  exit 1
fi

if ! grep -Fq "2026-06-15" "$audit_doc"; then
  echo "::error::${audit_doc} is missing the 2026-06-15 batch marker" >&2
  exit 1
fi

set +e
governed_output="$(
  python - <<'PY'
from pathlib import Path
import re
import sys

root = Path("crates/ntgw-ai/src")
pattern = re.compile(r"unwrap\(|expect\(")
module_decl = re.compile(
    r"(?:(?:pub(?:\([^)]*\))?)\s+)?mod\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b"
)


def module_roots(path: Path):
    if path.name in {"lib.rs", "main.rs", "mod.rs"}:
        return path.parent
    return path.parent / path.stem


def cfg_test_module_targets(path: Path, module_name: str):
    base = module_roots(path)
    module_dir = base / module_name
    return {base / f"{module_name}.rs", module_dir / "mod.rs"}, {module_dir}


def char_literal_end(chars, start: int):
    i = start + 1
    while i < len(chars):
        ch = chars[i]
        if ch == "\\":
            i += 2
            continue
        if ch == "'":
            return i
        if ch.isspace():
            return None
        i += 1
    return None


def sanitize_lines(lines):
    sanitized = []
    block_comment_depth = 0
    in_string = False
    string_escape = False
    in_raw_string = False
    raw_hashes = 0

    for line in lines:
        chars = list(line)
        out = chars[:]
        i = 0
        while i < len(chars):
            ch = chars[i]
            nxt = chars[i + 1] if i + 1 < len(chars) else ""

            if block_comment_depth > 0:
                out[i] = " "
                if ch == "/" and nxt == "*":
                    out[i + 1] = " "
                    block_comment_depth += 1
                    i += 2
                    continue
                if ch == "*" and nxt == "/":
                    out[i + 1] = " "
                    block_comment_depth -= 1
                    i += 2
                    continue
                i += 1
                continue

            if in_string:
                out[i] = " "
                if string_escape:
                    string_escape = False
                elif ch == "\\":
                    string_escape = True
                elif ch == '"':
                    in_string = False
                i += 1
                continue

            if in_raw_string:
                out[i] = " "
                if ch == '"' and chars[i + 1 : i + 1 + raw_hashes] == ["#"] * raw_hashes:
                    for j in range(i + 1, min(len(chars), i + 1 + raw_hashes)):
                        out[j] = " "
                    in_raw_string = False
                    i += 1 + raw_hashes
                    continue
                i += 1
                continue

            if ch == "/" and nxt == "/":
                for j in range(i, len(chars)):
                    out[j] = " "
                break

            if ch == "/" and nxt == "*":
                out[i] = " "
                out[i + 1] = " "
                block_comment_depth = 1
                i += 2
                continue

            if ch == "r":
                j = i + 1
                while j < len(chars) and chars[j] == "#":
                    j += 1
                if j < len(chars) and chars[j] == '"':
                    for k in range(i, j + 1):
                        out[k] = " "
                    in_raw_string = True
                    raw_hashes = j - (i + 1)
                    i = j + 1
                    continue

            if ch == "b" and nxt == "r":
                j = i + 2
                while j < len(chars) and chars[j] == "#":
                    j += 1
                if j < len(chars) and chars[j] == '"':
                    for k in range(i, j + 1):
                        out[k] = " "
                    in_raw_string = True
                    raw_hashes = j - (i + 2)
                    i = j + 1
                    continue

            if ch == "b" and nxt == '"':
                out[i] = " "
                out[i + 1] = " "
                in_string = True
                string_escape = False
                i += 2
                continue

            if ch == '"':
                out[i] = " "
                in_string = True
                string_escape = False
                i += 1
                continue

            if ch == "'":
                end = char_literal_end(chars, i)
                if end is not None:
                    for j in range(i, end + 1):
                        out[j] = " "
                    i = end + 1
                    continue

            i += 1

        sanitized.append("".join(out))

    return sanitized


def skip_item_state(line: str, brace_depth: int):
    next_depth = brace_depth + line.count("{") - line.count("}")
    if next_depth > 0:
        return True, next_depth
    if brace_depth > 0 and next_depth <= 0:
        return False, 0
    if "{" in line or ";" in line:
        return False, 0
    return True, 0


def collect_cfg_test_module_targets(path: Path, sanitized_lines):
    pending_test_attr = False
    excluded_paths = set()
    excluded_dirs = set()

    for line in sanitized_lines:
        stripped = line.lstrip()
        if stripped.startswith("#[cfg(test)]"):
            pending_test_attr = True
            continue
        if not pending_test_attr:
            continue
        if not stripped:
            continue
        if stripped.startswith("#["):
            continue
        module_match = module_decl.match(stripped)
        if module_match and ";" in line and "{" not in line:
            item_paths, item_dirs = cfg_test_module_targets(path, module_match.group("name"))
            excluded_paths.update(item_paths)
            excluded_dirs.update(item_dirs)
        pending_test_attr = False

    return excluded_paths, excluded_dirs


def production_lines(path: Path, lines, sanitized_lines):
    out = []
    skip_test_item = False
    brace_depth = 0
    pending_test_attr = False

    for lineno, (line, sanitized_line) in enumerate(zip(lines, sanitized_lines), 1):
        stripped = sanitized_line.lstrip()

        if skip_test_item:
            skip_test_item, brace_depth = skip_item_state(sanitized_line, brace_depth)
            continue

        if stripped.startswith("#[cfg(test)]"):
            pending_test_attr = True
            continue

        if pending_test_attr:
            if not stripped:
                continue
            if stripped.startswith("#["):
                continue
            pending_test_attr = False
            skip_test_item, brace_depth = skip_item_state(sanitized_line, 0)
            continue

        if pattern.search(sanitized_line):
            out.append((lineno, line))

    return out


matches = []
excluded_paths = set()
excluded_dirs = set()
files = sorted(root.rglob("*.rs"))
file_cache = {}

for path in files:
    lines = path.read_text().splitlines()
    sanitized_lines = sanitize_lines(lines)
    file_cache[path] = (lines, sanitized_lines)
    file_excluded_paths, file_excluded_dirs = collect_cfg_test_module_targets(
        path, sanitized_lines
    )
    excluded_paths.update(file_excluded_paths)
    excluded_dirs.update(file_excluded_dirs)

for path in files:
    if path in excluded_paths:
        continue
    if any(parent in excluded_dirs for parent in [path, *path.parents]):
        continue
    if "/tests/" in str(path):
        continue
    lines, sanitized_lines = file_cache[path]
    file_matches = production_lines(path, lines, sanitized_lines)
    for lineno, line in file_matches:
        matches.append(f"{path}:{lineno}:{line}")

if matches:
    print("\n".join(matches))
    sys.exit(1)
print("clean")
PY
)"
status=$?
set -e

printf '%s\n' "$governed_output"

if [[ "$mode" == "enforce" && $status -ne 0 ]]; then
  exit "$status"
fi
