#!/usr/bin/env python3

from __future__ import annotations

import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name("audit-runtime-unwraps.sh")
AI_DOC_PATH = Path(__file__).resolve().parent.parent / "docs" / "runtime-unwrap-ntgw-ai-zero-tolerance.md"
STREAM_DOC_FIXTURE = """# ntgw-stream Runtime Unwrap Governance

Date: 2026-06-15

This note records the multi-target guardrail expansion for `ntgw-stream`.
Governed scope is `crates/ntgw-stream/src/` production code only.
Inline `#[cfg(test)]` items/modules and files reached only through
`#[cfg(test)] mod ...;` remain excluded from enforcement.
"""


@dataclass
class Case:
    name: str
    files: dict[str, str | bytes]
    report_code: int
    enforce_code: int
    docs: dict[str, str] = field(default_factory=dict)
    report_contains: list[str] = field(default_factory=list)
    report_not_contains: list[str] = field(default_factory=list)
    enforce_contains: list[str] = field(default_factory=list)
    stderr_contains: list[str] = field(default_factory=list)


def write_fixture(root: Path, relpath: str, content: str | bytes) -> None:
    path = root / relpath
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(content, bytes):
        path.write_bytes(content)
    else:
        path.write_text(content)


def run_mode(repo: Path, mode: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", "scripts/audit-runtime-unwraps.sh", mode],
        cwd=repo,
        capture_output=True,
        text=True,
    )


def assert_case(case: Case) -> None:
    with tempfile.TemporaryDirectory() as td:
        repo = Path(td) / "repo"
        (repo / "scripts").mkdir(parents=True)
        (repo / "docs").mkdir()
        (repo / "scripts" / "audit-runtime-unwraps.sh").write_text(SCRIPT_PATH.read_text())

        default_docs = {
            "docs/runtime-unwrap-ntgw-ai-zero-tolerance.md": AI_DOC_PATH.read_text(),
            "docs/runtime-unwrap-ntgw-stream-zero-tolerance.md": STREAM_DOC_FIXTURE,
        }
        for relpath, content in {**default_docs, **case.docs}.items():
            write_fixture(repo, relpath, content)

        for relpath, content in case.files.items():
            write_fixture(repo, relpath, content)

        report = run_mode(repo, "--report")
        enforce = run_mode(repo, "--enforce")

        if report.returncode != case.report_code:
            raise AssertionError(
                f"{case.name}: expected report code {case.report_code}, got {report.returncode}\n"
                f"stdout:\n{report.stdout}\nstderr:\n{report.stderr}"
            )
        if enforce.returncode != case.enforce_code:
            raise AssertionError(
                f"{case.name}: expected enforce code {case.enforce_code}, got {enforce.returncode}\n"
                f"stdout:\n{enforce.stdout}\nstderr:\n{enforce.stderr}"
            )

        for needle in case.report_contains:
            if needle not in report.stdout:
                raise AssertionError(
                    f"{case.name}: expected report output to contain {needle!r}\n"
                    f"stdout:\n{report.stdout}\nstderr:\n{report.stderr}"
                )
        for needle in case.enforce_contains:
            if needle not in enforce.stdout:
                raise AssertionError(
                    f"{case.name}: expected enforce output to contain {needle!r}\n"
                    f"stdout:\n{enforce.stdout}\nstderr:\n{enforce.stderr}"
                )
        for needle in case.report_not_contains:
            if needle in report.stdout:
                raise AssertionError(
                    f"{case.name}: expected report output to omit {needle!r}\n"
                    f"stdout:\n{report.stdout}\nstderr:\n{report.stderr}"
                )
        for needle in case.stderr_contains:
            if needle not in report.stderr or needle not in enforce.stderr:
                raise AssertionError(
                    f"{case.name}: expected stderr to contain {needle!r}\n"
                    f"report stderr:\n{report.stderr}\nenforce stderr:\n{enforce.stderr}"
                )


def main() -> int:
    cases = [
        Case(
            name="multi_target_clean_repo_reports_each_target",
            files={
                "crates/ntgw-ai/src/lib.rs": "pub fn ai_ok() {}\n",
                "crates/ntgw-stream/src/lib.rs": "pub fn stream_ok() {}\n",
            },
            docs={"docs/runtime-unwrap-ntgw-stream-zero-tolerance.md": STREAM_DOC_FIXTURE},
            report_code=0,
            enforce_code=0,
            report_contains=[
                "== ntgw-ai (crates/ntgw-ai/src) ==",
                "== ntgw-stream (crates/ntgw-stream/src) ==",
                "clean",
            ],
            enforce_contains=[
                "== ntgw-ai (crates/ntgw-ai/src) ==",
                "== ntgw-stream (crates/ntgw-stream/src) ==",
                "clean",
            ],
        ),
        Case(
            name="multi_target_second_crate_production_match_detected",
            files={
                "crates/ntgw-ai/src/lib.rs": "pub fn ai_ok() {}\n",
                "crates/ntgw-stream/src/lib.rs": "pub fn stream_bad() { let _ = Some(1).unwrap(); }\n",
            },
            docs={"docs/runtime-unwrap-ntgw-stream-zero-tolerance.md": STREAM_DOC_FIXTURE},
            report_code=0,
            enforce_code=1,
            report_contains=[
                "== ntgw-stream (crates/ntgw-stream/src) ==",
                "crates/ntgw-stream/src/lib.rs:1:pub fn stream_bad() { let _ = Some(1).unwrap(); }",
            ],
            enforce_contains=[
                "== ntgw-stream (crates/ntgw-stream/src) ==",
                "crates/ntgw-stream/src/lib.rs:1:pub fn stream_bad() { let _ = Some(1).unwrap(); }",
            ],
        ),
        Case(
            name="clean_repo",
            files={"crates/ntgw-ai/src/lib.rs": "pub fn ok() {}\n"},
            report_code=0,
            enforce_code=0,
            report_contains=["clean"],
            enforce_contains=["clean"],
        ),
        Case(
            name="inline_cfg_item_excluded",
            files={
                "crates/ntgw-ai/src/lib.rs": "#[cfg(test)]\nfn helper() { let _ = Some(1).unwrap(); }\n"
            },
            report_code=0,
            enforce_code=0,
            report_contains=["clean"],
        ),
        Case(
            name="whitespace_calls_detected",
            files={
                "crates/ntgw-ai/src/lib.rs": (
                    "fn x() {\n"
                    "    let _ = Some(1).unwrap ();\n"
                    "    let _ = Some(2).expect (\"boom\");\n"
                    "}\n"
                )
            },
            report_code=0,
            enforce_code=1,
            report_contains=[
                'crates/ntgw-ai/src/lib.rs:2:    let _ = Some(1).unwrap ();',
                'crates/ntgw-ai/src/lib.rs:3:    let _ = Some(2).expect ("boom");',
            ],
        ),
        Case(
            name="multiline_calls_detected",
            files={
                "crates/ntgw-ai/src/lib.rs": (
                    "fn production_probe() {\n"
                    "    let _ = Some(1).unwrap\n"
                    "    ();\n"
                    "    let _ = Some(2).expect\n"
                    "    (\"boom\");\n"
                    "}\n"
                )
            },
            report_code=0,
            enforce_code=1,
            report_contains=[
                "crates/ntgw-ai/src/lib.rs:2:    let _ = Some(1).unwrap",
                "crates/ntgw-ai/src/lib.rs:4:    let _ = Some(2).expect",
            ],
        ),
        Case(
            name="same_line_non_test_attr_reported",
            files={
                "crates/ntgw-ai/src/lib.rs": '#[inline] pub fn production_inline_attr() { let _ = Some(1).expect("prod"); }\n'
            },
            report_code=0,
            enforce_code=1,
            report_contains=[
                'crates/ntgw-ai/src/lib.rs:1:#[inline] pub fn production_inline_attr() { let _ = Some(1).expect("prod"); }'
            ],
        ),
        Case(
            name="multiline_non_test_attr_reported",
            files={
                "crates/ntgw-ai/src/__audit_review_tmp.rs": (
                    "#[allow(\n"
                    "    dead_code\n"
                    ')] pub fn review_tmp() { Option::<u8>::None.expect("prod"); }\n'
                    "pub fn next_line() {}\n"
                )
            },
            report_code=0,
            enforce_code=1,
            report_contains=[
                'crates/ntgw-ai/src/__audit_review_tmp.rs:3:)] pub fn review_tmp() { Option::<u8>::None.expect("prod"); }'
            ],
            report_not_contains=["pub fn next_line() {}"],
        ),
        Case(
            name="cfg_test_external_module_excluded",
            files={
                "crates/ntgw-ai/src/lib.rs": '#[cfg(test)]\npub(crate) mod tests;\nlet _ = Some(3).expect("prod");\n',
                "crates/ntgw-ai/src/tests.rs": "let _ = Some(1).unwrap();\n",
                "crates/ntgw-ai/src/tests/mod.rs": "let _ = Some(2).unwrap();\n",
            },
            report_code=0,
            enforce_code=1,
            report_contains=['crates/ntgw-ai/src/lib.rs:3:let _ = Some(3).expect("prod");'],
            report_not_contains=["tests.rs:1", "tests/mod.rs:1"],
        ),
        Case(
            name="cfg_test_path_module_excluded",
            files={
                "crates/ntgw-ai/src/lib.rs": '#[cfg(test)] #[path = "tests/mod.rs"] mod tests;\nlet _ = Some(3).expect("prod");\n',
                "crates/ntgw-ai/src/tests/mod.rs": "let _ = Some(1).unwrap();\n",
            },
            report_code=0,
            enforce_code=1,
            report_contains=['crates/ntgw-ai/src/lib.rs:2:let _ = Some(3).expect("prod");'],
            report_not_contains=["tests/mod.rs:1"],
        ),
        Case(
            name="multiline_cfg_external_module_excluded",
            files={
                "crates/ntgw-ai/src/lib.rs": '#[cfg(\n    test\n)] mod tests;\nlet _ = Some(3).expect("prod");\n',
                "crates/ntgw-ai/src/tests.rs": "let _ = Some(1).unwrap();\n",
            },
            report_code=0,
            enforce_code=1,
            report_contains=['crates/ntgw-ai/src/lib.rs:4:let _ = Some(3).expect("prod");'],
            report_not_contains=["tests.rs:1"],
        ),
        Case(
            name="multiline_cfg_with_path_line_excluded",
            files={
                "crates/ntgw-ai/src/lib.rs": (
                    "#[cfg(test)]\n"
                    '#[path = "custom/alt_tests.rs"]\n'
                    "mod custom_tests;\n"
                    'let _ = Some(3).expect("prod");\n'
                ),
                "crates/ntgw-ai/src/custom/alt_tests.rs": "let _ = Some(1).unwrap();\n",
            },
            report_code=0,
            enforce_code=1,
            report_contains=['crates/ntgw-ai/src/lib.rs:4:let _ = Some(3).expect("prod");'],
            report_not_contains=["custom/alt_tests.rs:1"],
        ),
        Case(
            name="production_src_tests_module_scanned",
            files={
                "crates/ntgw-ai/src/lib.rs": "pub mod tests;\n",
                "crates/ntgw-ai/src/tests/mod.rs": "fn prod_like() { let _ = Some(1).unwrap(); }\n",
            },
            report_code=0,
            enforce_code=1,
            report_contains=[
                "crates/ntgw-ai/src/tests/mod.rs:1:fn prod_like() { let _ = Some(1).unwrap(); }"
            ],
        ),
        Case(
            name="string_literal_ignored",
            files={"crates/ntgw-ai/src/lib.rs": 'const EXAMPLE: &str = "expect(";\n'},
            report_code=0,
            enforce_code=0,
            report_contains=["clean"],
        ),
        Case(
            name="invalid_utf8_errors",
            files={"crates/ntgw-ai/src/bad.rs": b"\xff"},
            report_code=2,
            enforce_code=2,
            stderr_contains=["::error::runtime unwrap scanner failed:"],
        ),
    ]

    for case in cases:
        assert_case(case)
        print(f"ok - {case.name}")

    print(f"passed {len(cases)} audit scanner fixture cases")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AssertionError as exc:
        print(exc, file=sys.stderr)
        raise SystemExit(1)
