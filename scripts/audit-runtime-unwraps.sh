#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/audit-runtime-unwraps.sh [--report|--enforce]

Reports the runtime unwrap governance state for governed production sources.
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

set +e
governed_output="$(
  python - <<'PY'
from dataclasses import dataclass
from pathlib import Path
import re
import sys


@dataclass(frozen=True)
class GovernedTarget:
    name: str
    root: Path
    doc: Path
    marker: str


class TargetValidationError(Exception):
    pass


targets = [
    GovernedTarget(
        name="ntgw-ai",
        root=Path("crates/ntgw-ai/src"),
        doc=Path("docs/runtime-unwrap-ntgw-ai-zero-tolerance.md"),
        marker="2026-06-15",
    ),
    GovernedTarget(
        name="ntgw-stream",
        root=Path("crates/ntgw-stream/src"),
        doc=Path("docs/runtime-unwrap-ntgw-stream-zero-tolerance.md"),
        marker="2026-06-15",
    ),
]

pattern = re.compile(r"(?<![A-Za-z0-9_])(?:unwrap|expect)\s*\(")
cfg_test_attr = re.compile(r"^#\[\s*cfg\s*\(\s*test\s*\)\s*\]$")
path_attr = re.compile(r'^#\[\s*path\s*=\s*"([^"]+)"\s*\]$')
module_decl = re.compile(
    r"(?:(?:pub(?:\([^)]*\))?)\s+)?mod\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b"
)


def module_roots(path: Path):
    if path.name in {"lib.rs", "main.rs", "mod.rs"}:
        return path.parent
    return path.parent / path.stem


def cfg_test_module_targets(path: Path, module_name: str, explicit_path: str | None = None):
    if explicit_path is not None:
        target = (path.parent / explicit_path).resolve().relative_to(Path.cwd())
        paths = {target}
        dirs = {target.parent} if target.name == "mod.rs" else set()
        return paths, dirs

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


def cfg_test_attribute_matches(parts):
    return cfg_test_attr.match(" ".join(parts)) is not None


def path_attribute_value(parts):
    for part in parts:
        match = path_attr.match(part)
        if match is not None:
            return match.group(1)
    return None


def split_single_line_attributes(text):
    attrs = []
    rest = text

    while rest.startswith("#["):
        depth = 0
        i = 2
        while i < len(rest):
            ch = rest[i]
            if ch == "[":
                depth += 1
            elif ch == "]":
                if depth == 0:
                    attrs.append(rest[: i + 1].strip())
                    rest = rest[i + 1 :].lstrip()
                    break
                depth -= 1
            i += 1
        else:
            return None, None

    return attrs, rest


def collect_cfg_test_module_targets(path: Path, lines, sanitized_lines):
    pending_test_attr = False
    pending_test_path = None
    buffered_original = None
    buffered_sanitized = None
    excluded_paths = set()
    excluded_dirs = set()

    for line, sanitized_line in zip(lines, sanitized_lines):
        stripped = line.strip()
        sanitized_stripped = sanitized_line.strip()

        if buffered_sanitized is not None:
            combined_original = " ".join(
                part for part in (buffered_original, stripped) if part
            )
            combined_sanitized = " ".join(
                part for part in (buffered_sanitized, sanitized_stripped) if part
            )
            attrs, rest = split_single_line_attributes(combined_original)
            sanitized_attrs, sanitized_rest = split_single_line_attributes(combined_sanitized)
            if attrs is None or sanitized_attrs is None:
                buffered_original = combined_original
                buffered_sanitized = combined_sanitized
                continue
            buffered_original = None
            buffered_sanitized = None
            if any(cfg_test_attribute_matches([attr]) for attr in sanitized_attrs):
                pending_test_attr = True
            explicit_path = path_attribute_value(attrs)
            if explicit_path is not None:
                pending_test_path = explicit_path
            stripped = rest
            sanitized_stripped = sanitized_rest

        elif sanitized_stripped.startswith("#["):
            attrs, rest = split_single_line_attributes(stripped)
            sanitized_attrs, sanitized_rest = split_single_line_attributes(sanitized_stripped)
            if attrs is None or sanitized_attrs is None:
                buffered_original = stripped
                buffered_sanitized = sanitized_stripped
                continue
            if any(cfg_test_attribute_matches([attr]) for attr in sanitized_attrs):
                pending_test_attr = True
            explicit_path = path_attribute_value(attrs)
            if explicit_path is not None:
                pending_test_path = explicit_path
            stripped = rest
            sanitized_stripped = sanitized_rest
            if not stripped:
                continue
            if not pending_test_attr:
                continue
        if not pending_test_attr:
            continue
        if not sanitized_stripped:
            continue
        module_match = module_decl.match(sanitized_stripped)
        if module_match and ";" in sanitized_stripped and "{" not in sanitized_stripped:
            item_paths, item_dirs = cfg_test_module_targets(
                path,
                module_match.group("name"),
                pending_test_path,
            )
            excluded_paths.update(item_paths)
            excluded_dirs.update(item_dirs)
        pending_test_attr = False
        pending_test_path = None

    return excluded_paths, excluded_dirs


def production_lines(path: Path, lines, sanitized_lines):
    masked_lines = []
    skip_test_item = False
    brace_depth = 0
    pending_test_attr = False
    buffered_sanitized = None

    for sanitized_line in sanitized_lines:
        stripped = sanitized_line.strip()
        masked_line = " " * len(sanitized_line)

        if skip_test_item:
            masked_lines.append(masked_line)
            skip_test_item, brace_depth = skip_item_state(sanitized_line, brace_depth)
            continue

        if buffered_sanitized is not None:
            combined_sanitized = " ".join(
                part for part in (buffered_sanitized, stripped) if part
            )
            attrs, rest = split_single_line_attributes(combined_sanitized)
            if attrs is None:
                masked_lines.append(masked_line)
                buffered_sanitized = combined_sanitized
                continue
            buffered_sanitized = None
            if any(cfg_test_attribute_matches([attr]) for attr in attrs):
                pending_test_attr = True
            stripped = rest
            sanitized_line = rest
            masked_line = " " * len(rest)
            if not stripped:
                masked_lines.append(" " * len(sanitized_line))
                continue
            if pending_test_attr:
                masked_lines.append(" " * len(sanitized_line))
                pending_test_attr = False
                skip_test_item, brace_depth = skip_item_state(rest, 0)
                continue

        if stripped.startswith("#["):
            attrs, rest = split_single_line_attributes(stripped)
            if attrs is None:
                masked_lines.append(masked_line)
                buffered_sanitized = stripped
                continue
            if any(cfg_test_attribute_matches([attr]) for attr in attrs):
                pending_test_attr = True
            if not rest:
                masked_lines.append(masked_line)
                continue
            if pending_test_attr:
                masked_lines.append(masked_line)
                pending_test_attr = False
                skip_test_item, brace_depth = skip_item_state(rest, 0)
                continue
            sanitized_line = rest
            stripped = rest
            masked_line = " " * len(rest)

        if pending_test_attr:
            masked_lines.append(masked_line)
            if not stripped:
                continue
            pending_test_attr = False
            skip_test_item, brace_depth = skip_item_state(sanitized_line, 0)
            continue

        masked_lines.append(sanitized_line)

    matches = []
    masked_text = "\n".join(masked_lines)
    seen_line_numbers = set()

    for match in pattern.finditer(masked_text):
        line_number = masked_text.count("\n", 0, match.start()) + 1
        if line_number in seen_line_numbers:
            continue
        seen_line_numbers.add(line_number)
        matches.append((line_number, lines[line_number - 1]))

    return matches


def scan_target(root: Path):
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
            path, lines, sanitized_lines
        )
        excluded_paths.update(file_excluded_paths)
        excluded_dirs.update(file_excluded_dirs)

    for path in files:
        if path in excluded_paths:
            continue
        if any(parent in excluded_dirs for parent in [path, *path.parents]):
            continue
        lines, sanitized_lines = file_cache[path]
        file_matches = production_lines(path, lines, sanitized_lines)
        for lineno, line in file_matches:
            matches.append(f"{path}:{lineno}:{line}")

    return matches


try:
    results = []

    for target in targets:
        if not target.doc.is_file():
            raise TargetValidationError(f"{target.doc} is missing")
        if target.marker not in target.doc.read_text():
            raise TargetValidationError(
                f"{target.doc} is missing the {target.marker} batch marker"
            )
        results.append((target, scan_target(target.root)))

    failed = False
    for index, (target, matches) in enumerate(results):
        if index:
            print()
        print(f"== {target.name} ({target.root}) ==")
        if matches:
            failed = True
            print("\n".join(matches))
        else:
            print("clean")

    if failed:
        sys.exit(1)
except TargetValidationError as exc:
    print(f"::error::{exc}", file=sys.stderr)
    sys.exit(1)
except Exception as exc:
    print(f"::error::runtime unwrap scanner failed: {exc}", file=sys.stderr)
    sys.exit(2)
PY
)"
status=$?
set -e

printf '%s\n' "$governed_output"

if [[ $status -ne 0 && ( "$mode" == "enforce" || $status -ne 1 ) ]]; then
  exit "$status"
fi
