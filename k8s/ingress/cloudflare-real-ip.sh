#!/bin/bash
# Make the shared ingress-nginx trust Cloudflare's CF-Connecting-IP only when the
# connection really comes from Cloudflare, so no caller can forge its address.
#
# Why: ingress-nginx (namespace ingress-nginx, installed by kubectl apply, not in
# this repo) serves Oya, a2abaseai and sitetwin on one LoadBalancer. Its ConfigMap
# reads CF-Connecting-IP with no proxy-real-ip-cidr, so it believed that header
# from anyone, and hosts that skip Cloudflare (sitetwin.dev, browser.getoya.ai, ...)
# could name any address. With externalTrafficPolicy Local nginx sees the true TCP
# peer; with proxy-real-ip-cidr set to Cloudflare's ranges it swaps in
# CF-Connecting-IP only for Cloudflare peers. X-Real-IP (what the Oya server reads,
# OYA_CLIENT_IP_HEADER) then always holds the real caller.
#
# Uses kubectl patch, never apply: the ConfigMap holds other keys (a2abaseai's
# chat_completions limit) that a partial apply would delete.
#
# Cloudflare ranges: https://www.cloudflare.com/ips-v4 and /ips-v6 (fetched
# 2026-11-20). If Cloudflare adds a range, visitors on it share that edge's
# rate-limit bucket until this list is updated; requests still work.
#
# Usage: ./cloudflare-real-ip.sh check | apply | rollback | status
#   check: server-side dry run of both patches, nothing changes.
# Order matters: the Service goes to Local BEFORE the ConfigMap gets the ranges.
# The other way round, nginx would see node IPs (not Cloudflare), stop trusting
# CF-Connecting-IP and put every visitor in one of three buckets. The controller
# is scaled to one pod per node first so no node drops traffic under Local.
# Expect a short blip; long-lived websockets close when old nginx workers drain.
#
# A later `kubectl apply` of the controller manifest would put the Service back
# to Cluster (its last-applied config says so) while the ranges stay: `status`
# warns about that. Put externalTrafficPolicy: Local in that manifest too.
set -euo pipefail

CONTEXT=gke_a2abaseai_us-central1_a2abaseai-gke
NS=ingress-nginx
REPLICAS=3
CM=ingress-nginx-controller
SVC=ingress-nginx-controller
CLOUDFLARE_RANGES="173.245.48.0/20,103.21.244.0/22,103.22.200.0/22,103.31.4.0/22,141.101.64.0/18,108.162.192.0/18,190.93.240.0/20,188.114.96.0/20,197.234.240.0/22,198.41.128.0/17,162.158.0.0/15,104.16.0.0/13,104.24.0.0/14,172.64.0.0/13,131.0.72.0/22,2400:cb00::/32,2606:4700::/32,2803:f800::/32,2405:b500::/32,2405:8100::/32,2a06:98c0::/29,2c0f:f248::/32"

k() { kubectl --context "$CONTEXT" -n "$NS" "$@"; }

CM_PATCH="{\"data\":{\"proxy-real-ip-cidr\":\"$CLOUDFLARE_RANGES\"}}"
SVC_LOCAL='{"spec":{"externalTrafficPolicy":"Local"}}'
SVC_CLUSTER='{"spec":{"externalTrafficPolicy":"Cluster"}}'

status() {
  local cidr policy
  cidr=$(k get cm "$CM" -o jsonpath='{.data.proxy-real-ip-cidr}')
  policy=$(k get svc "$SVC" -o jsonpath='{.spec.externalTrafficPolicy}')
  echo "proxy-real-ip-cidr: ${cidr:-<unset>}"
  echo "externalTrafficPolicy: $policy"
  echo "controller replicas ready: $(k get deploy "$CM" -o jsonpath='{.status.readyReplicas}/{.spec.replicas}')"
  if [ -n "$cidr" ] && [ "$policy" != "Local" ]; then
    echo "WARNING: ranges set but policy is $policy: every visitor shares a node bucket. Run apply or rollback." >&2
  fi
}

check() {
  k patch svc "$SVC" --type merge -p "$SVC_LOCAL" --dry-run=server
  k patch cm "$CM" --type merge -p "$CM_PATCH" --dry-run=server
}

apply() {
  k scale deploy "$CM" --replicas="$REPLICAS"
  k rollout status deploy "$CM" --timeout=180s
  k patch svc "$SVC" --type merge -p "$SVC_LOCAL"
  echo "Waiting 45s for the load balancer health checks to settle..."
  sleep 45
  k patch cm "$CM" --type merge -p "$CM_PATCH"
  status
}

rollback() {
  if [ -n "$(k get cm "$CM" -o jsonpath='{.data.proxy-real-ip-cidr}')" ]; then
    k patch cm "$CM" --type json -p '[{"op":"remove","path":"/data/proxy-real-ip-cidr"}]'
  fi
  k patch svc "$SVC" --type merge -p "$SVC_CLUSTER"
  status
}

case "${1:-}" in
  check) check ;;
  apply) apply ;;
  rollback) rollback ;;
  status) status ;;
  *) echo "Usage: $0 check | apply | rollback | status" >&2; exit 1 ;;
esac
