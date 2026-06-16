#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${BUF_INPUT:-}" ]]; then
  input="$BUF_INPUT"
elif [[ -f ../proto/buf.yaml && -f ../proto/gateway/control/v1/control.proto ]]; then
  input="../proto"
else
  input="buf.build/nantian/proto"
fi

buf generate "$input"

if ! git diff --exit-code crates/ntgw-proto/src/bsr/; then
  echo "::error::BSR-generated Rust code is out of date. Run 'buf generate buf.build/nantian/proto' from the dataplane repository and commit the updated files."
  exit 1
fi
