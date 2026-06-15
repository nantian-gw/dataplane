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
module_decl = re.compile(r"(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\b")


def production_lines(path: Path):
    lines = path.read_text().splitlines()
    out = []
    skip_module = False
    brace_depth = 0
    pending_test_attr = False

    for lineno, line in enumerate(lines, 1):
        stripped = line.lstrip()

        if skip_module:
            brace_depth += line.count("{") - line.count("}")
            if brace_depth <= 0:
                skip_module = False
                brace_depth = 0
            continue

        if stripped.startswith("#[cfg(test)]"):
            pending_test_attr = True
            continue

        if pending_test_attr:
            if not stripped:
                continue
            if stripped.startswith("#["):
                continue
            if module_decl.match(stripped):
                skip_module = True
                brace_depth = line.count("{") - line.count("}")
                if brace_depth <= 0:
                    skip_module = False
                    brace_depth = 0
                pending_test_attr = False
                continue
            pending_test_attr = False

        if pattern.search(line):
            out.append((lineno, line))

    return out


matches = []
for path in sorted(root.rglob("*.rs")):
    if "/tests/" in str(path):
        continue
    for lineno, line in production_lines(path):
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
