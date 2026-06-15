#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/audit-runtime-unwraps.sh [--report|--enforce]

Reports the first-batch runtime unwrap governance state for ntgw-ai.
`--enforce` fails if any governed production file still contains unwrap/expect.
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

audit_doc="docs/runtime-unwrap-first-batch.md"
if [[ ! -f "$audit_doc" ]]; then
  echo "::error::${audit_doc} is missing" >&2
  exit 1
fi

for bucket in runtime config invariant test; do
  if ! grep -Fq "| ${bucket} |" "$audit_doc"; then
    echo "::error::${audit_doc} is missing the ${bucket} bucket" >&2
    exit 1
  fi
done

pattern='unwrap\(|expect\('

governed_targets=(
  'langfuse-basic-auth|crates/ntgw-ai/src/observability/langfuse.rs|.expect("valid basic auth header — base64 output is always ASCII-safe");'
  'langfuse-client-build|crates/ntgw-ai/src/observability/langfuse.rs|.expect("reqwest client build with valid default headers")'
  'openai-multipart|crates/ntgw-ai/src/format/openai.rs|Some(serde_json::to_value(parts).expect("MultiPart serialization should not fail"))'
  'openai-error-body|crates/ntgw-ai/src/format/openai.rs|Ok(serde_json::to_vec(&error).unwrap())'
  'ollama-error-body|crates/ntgw-ai/src/format/ollama.rs|Ok(serde_json::to_vec(&error).unwrap())'
  'anthropic-texts|crates/ntgw-ai/src/format/anthropic.rs|AIContent::Text(texts.into_iter().next().expect("non-empty texts"))'
  'anthropic-stop-json|crates/ntgw-ai/src/format/anthropic.rs|let stop_json = serde_json::to_string(&stop).unwrap();'
  'anthropic-error-body|crates/ntgw-ai/src/format/anthropic.rs|Ok(serde_json::to_vec(&error).unwrap())'
)

deferred_files=(
  "crates/ntgw-ai/src/filter.rs"
  "crates/ntgw-ai/src/content_safety.rs"
  "crates/ntgw-ai/src/prompt_guard.rs"
  "crates/ntgw-ai/src/pii.rs"
  "crates/ntgw-stream/src/pool.rs"
  "crates/ntgw-http/src/proxy/logging.rs"
)

echo "== Governed files (must be zero after implementation) =="
governed_output="$(
  for target in "${governed_targets[@]}"; do
    IFS='|' read -r _label file snippet <<<"$target"
    grep -HFn "$snippet" "$file" || true
  done | while IFS= read -r match; do
    [[ -n "$match" ]] || continue
    printf '%s\n' "$match"
  done
)"
if [[ -n "$governed_output" ]]; then
  printf '%s\n' "$governed_output"
  if [[ "$mode" == "enforce" ]]; then
    echo "::error::governed runtime/config files still contain unwrap/expect" >&2
    exit 1
  fi
else
  echo "clean"
fi

echo
echo "== Deferred sample files (documented, not enforced in batch 1) =="
rg -n "$pattern" "${deferred_files[@]}" || true
