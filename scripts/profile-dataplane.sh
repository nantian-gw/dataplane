#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CLUSTER_NAME="${CLUSTER_NAME:-perf-profile}"
PROFILE_DURATION="${PROFILE_DURATION:-30}"
GATEWAY_DIR="$REPO_ROOT/gateway"
OUTPUT_DIR="${OUTPUT_DIR:-$SCRIPT_DIR/profiles}"
TIMESTAMP="$(date -u +%Y%m%d-%H%M%S)"
mkdir -p "$OUTPUT_DIR"

echo "=== [1/5] Setup cluster ==="
cd "$GATEWAY_DIR"
export CLUSTER_NAME BIN_DIR=/usr/local/bin

if ! kind get clusters 2>/dev/null | grep -qx "$CLUSTER_NAME"; then
  scripts/ci/create-kind-cluster.sh
  scripts/ci/install-gateway-api-crds.sh
fi

echo "=== [2/5] Load images into kind ==="
for img in ghcr.io/nantian-gw/nantian-controlplane:latest \
           ghcr.io/nantian-gw/dataplane:latest \
           gcr.io/k8s-staging-gateway-api/echo-basic:v20260204-monthly-2026.01-60-g28382302; do
  if docker image inspect "$img" &>/dev/null; then
    kind load docker-image "$img" --name "$CLUSTER_NAME" 2>/dev/null || true
    echo "  loaded: $img"
  fi
done

echo "=== [3/5] Deploy ==="
CONTROLPLANE_IMAGE=ghcr.io/nantian-gw/nantian-controlplane:latest \
  DATAPLANE_IMAGE=ghcr.io/nantian-gw/dataplane:latest \
  DASHBOARD_IMAGE=ghcr.io/nantian-gw/dashboard:latest \
  KIND_DATAPLANE_IMAGE=ghcr.io/nantian-gw/dataplane:latest \
  CONFORMANCE_EXPERIMENTAL=true ALL_FEATURES=true \
  bash "$GATEWAY_DIR/scripts/ci/deploy-kind-conformance.sh"
kubectl wait --for=condition=available --timeout=120s deployment/nantian-gw-controlplane -n nantian-gw 2>/dev/null || true
kubectl wait --for=condition=available --timeout=120s deployment/nantian-gw-dataplane -n nantian-gw 2>/dev/null || true
kubectl get pods -n nantian-gw

kubectl apply -f - <<'YAML'
apiVersion: v1
kind: Pod
metadata: {name: perf-backend, namespace: nantian-gw, labels: {app: perf-backend}}
spec:
  containers:
  - {name: echo, image: gcr.io/k8s-staging-gateway-api/echo-basic:v20260204-monthly-2026.01-60-g28382302, ports: [{containerPort: 3000}]}
---
apiVersion: v1
kind: Service
metadata: {name: perf-backend, namespace: nantian-gw}
spec:
  selector: {app: perf-backend}
  ports: [{port: 80, targetPort: 3000}]
---
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata: {name: perf-gw, namespace: nantian-gw}
spec:
  gatewayClassName: nantian-gw
  listeners: [{name: http, port: 80, protocol: HTTP}]
---
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata: {name: perf-route, namespace: nantian-gw}
spec:
  parentRefs: [{name: perf-gw}]
  rules: [{backendRefs: [{name: perf-backend, port: 80}]}]
YAML

kubectl wait --for=condition=ready pod/perf-backend -n nantian-gw --timeout=60s
for i in $(seq 1 30); do
  S=$(kubectl get gateway perf-gw -n nantian-gw -o json | jq -r '.status.conditions[]? | select(.type=="Programmed") | .status // ""')
  [[ "$S" == "True" ]] && break
  sleep 2
done

GW=$(kubectl get svc -n nantian-gw -o json | jq -r '.items[] | select(.metadata.name | startswith("nantian-gw-perf-gw")) | .metadata.name' | head -1)
echo "Gateway service: $GW"

echo "=== [4/5] Profile ($PROFILE_DURATION s) ==="
PERF_FILE="$OUTPUT_DIR/perf-$TIMESTAMP.data"
FLAMEGRAPH_FILE="$OUTPUT_DIR/flamegraph-$TIMESTAMP.svg"

sudo perf record -a -g -F 99 -o "$PERF_FILE" -- sleep "$PROFILE_DURATION" &
PERF_PID=$!
sleep 2

kubectl port-forward -n nantian-gw "svc/$GW" 18080:80 &
PF_PID=$!
sleep 2

END=$((SECONDS + PROFILE_DURATION - 5))
while [[ $SECONDS -lt $END ]]; do
  curl -s -o /dev/null http://localhost:18080/ &
done
wait 2>/dev/null || true
kill $PF_PID 2>/dev/null || true

wait $PERF_PID 2>/dev/null || true

echo "=== [5/5] Generate flamegraph ==="
sudo chmod 644 "$PERF_FILE" 2>/dev/null || true
if perf script -i "$PERF_FILE" 2>/dev/null | ~/.cargo/bin/inferno-collapse-perf 2>/dev/null | ~/.cargo/bin/inferno-flamegraph > "$FLAMEGRAPH_FILE" 2>/dev/null && [[ -s "$FLAMEGRAPH_FILE" ]]; then
  echo "Done: $FLAMEGRAPH_FILE"
else
  echo "Raw perf data: $PERF_FILE"
fi
