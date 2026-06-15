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
    if path.name == "mod.rs":
        return path.parent
    return path.parent / path.stem


def skip_item_state(line: str, brace_depth: int):
    next_depth = brace_depth + line.count("{") - line.count("}")
    if next_depth > 0:
        return True, next_depth
    if brace_depth > 0 and next_depth <= 0:
        return False, 0
    if "{" in line or ";" in line:
        return False, 0
    return True, 0


def cfg_test_module_targets(path: Path, module_name: str):
    base = module_roots(path)
    module_dir = base / module_name
    return {base / f"{module_name}.rs", module_dir / "mod.rs"}, {module_dir}


def production_lines(path: Path):
    lines = path.read_text().splitlines()
    out = []
    skip_test_item = False
    brace_depth = 0
    pending_test_attr = False
    excluded_paths = set()
    excluded_dirs = set()

    for lineno, line in enumerate(lines, 1):
        stripped = line.lstrip()

        if skip_test_item:
            skip_test_item, brace_depth = skip_item_state(line, brace_depth)
            continue

        if stripped.startswith("#[cfg(test)]"):
            pending_test_attr = True
            continue

        if pending_test_attr:
            if not stripped:
                continue
            if stripped.startswith("#["):
                continue
            module_match = module_decl.match(stripped)
            if module_match and ";" in line and "{" not in line:
                item_paths, item_dirs = cfg_test_module_targets(
                    path, module_match.group("name")
                )
                excluded_paths.update(item_paths)
                excluded_dirs.update(item_dirs)
            pending_test_attr = False
            skip_test_item, brace_depth = skip_item_state(line, 0)
            continue

        if pattern.search(line):
            out.append((lineno, line))

    return out, excluded_paths, excluded_dirs


matches = []
excluded_paths = set()
excluded_dirs = set()

for path in sorted(root.rglob("*.rs")):
    if path in excluded_paths:
        continue
    if any(parent in excluded_dirs for parent in [path, *path.parents]):
        continue
    if "/tests/" in str(path):
        continue
    file_matches, file_excluded_paths, file_excluded_dirs = production_lines(path)
    excluded_paths.update(file_excluded_paths)
    excluded_dirs.update(file_excluded_dirs)
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
