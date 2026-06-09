#!/usr/bin/env bash
set -euo pipefail

buf generate buf.build/nantian/proto

if ! git diff --exit-code crates/ntgw-proto/src/bsr/; then
  echo "::error::BSR-generated Rust code is out of date. Run 'buf generate buf.build/nantian/proto' from the dataplane repository and commit the updated files."
  exit 1
fi
