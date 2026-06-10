#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/verify-docker-build.sh [--static-only]

Runs Dockerfile static checks. Without --static-only, also builds the dataplane
image from a temporary synthetic context and runs `ntgw-app --help`.
USAGE
}

static_only=0
case "${1:-}" in
  "")
    ;;
  --static-only)
    static_only=1
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
dockerfile="${repo_root}/Dockerfile"
dockerignore="${repo_root}/Dockerfile.dockerignore"

require_contains() {
  local pattern="$1"
  local file="$2"
  if ! grep -Fq "$pattern" "$file"; then
    echo "::error::Expected ${file} to contain: ${pattern}" >&2
    exit 1
  fi
}

reject_contains() {
  local pattern="$1"
  local file="$2"
  if grep -Fq "$pattern" "$file"; then
    echo "::error::Unexpected ${file} content: ${pattern}" >&2
    exit 1
  fi
}

require_contains "cargo install cargo-chef --locked" "$dockerfile"
require_contains "cargo chef prepare --recipe-path recipe.json" "$dockerfile"
require_contains "cargo chef cook --release --recipe-path recipe.json -p ntgw-app" "$dockerfile"
reject_contains "COPY tests/testdata" "$dockerfile"
reject_contains "protobuf-compiler" "$dockerfile"

if [[ ! -f "$dockerignore" ]]; then
  echo "::error::Dockerfile.dockerignore is missing" >&2
  exit 1
fi

require_contains "dataplane/target" "$dockerignore"
require_contains "dataplane/.git" "$dockerignore"

if [[ "$static_only" -eq 1 ]]; then
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "::error::docker is required for full Docker build verification" >&2
  exit 1
fi

tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

mkdir -p "${tmp}/dataplane"
(
  cd "$repo_root"
  tar \
    --exclude='./target' \
    --exclude='./.git' \
    --exclude='./.idea' \
    --exclude='./.vscode' \
    -cf - .
) | tar -xf - -C "${tmp}/dataplane"

image_tag="${IMAGE_TAG:-ntgw-app:cargo-chef-local}"
features="${DATAPLANE_CARGO_FEATURES-allocator-jemalloc}"

docker build \
  -f "${tmp}/dataplane/Dockerfile" \
  --build-arg "DATAPLANE_CARGO_FEATURES=${features}" \
  -t "$image_tag" \
  "$tmp"

docker run --rm "$image_tag" --help >/dev/null
