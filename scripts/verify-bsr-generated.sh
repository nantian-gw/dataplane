#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd "$script_dir/.." && pwd -P)"

cd "$repo_root"

if [[ -n "${BUF_INPUT:-}" ]]; then
  input="$BUF_INPUT"
elif [[ -f "$repo_root/../proto/buf.yaml" && -f "$repo_root/../proto/gateway/control/v1/control.proto" ]]; then
  input="../proto"
else
  input="buf.build/nantian/proto"
fi

buf generate "$input"

if ! git diff --exit-code crates/ntgw-proto/src/bsr/; then
  echo "::error::BSR-generated Rust code is out of date. Run 'scripts/verify-bsr-generated.sh' from the dataplane repository or 'buf generate ../proto' when the sibling proto worktree is present, then commit the updated files."
  exit 1
fi
