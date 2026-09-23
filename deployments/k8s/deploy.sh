#!/usr/bin/env bash
# Deploy Oya to any Kubernetes cluster: control-plane replicas behind a
# Service, one pod per browser in the same namespace, access through a
# ServiceAccount and an RBAC Role. See README.md.
#
#   ./deploy.sh deploy --context CTX (--registry REPO | --image REF) (--database-url URL | --in-cluster-postgres) [options]
#   ./deploy.sh smoke | status | keys | logs   --context CTX [--namespace NS]
#   ./deploy.sh destroy --context CTX [--namespace NS] [--delete-data] [--yes]
#
# --context is always required, so nothing is ever applied to whichever cluster happens to be current.
#
# Options for deploy:
#   --namespace NS          (default oya)
#   --registry REPO         push the image built from this checkout here, e.g. ghcr.io/you/oya-control-plane
#   --image REF             use an already-pushed deployment image instead of building
#   --platform P            build for this platform (default linux/amd64)
#   --database-url URL      your managed Postgres (stored in the oya-secrets Secret)
#   --in-cluster-postgres   run Postgres in the cluster (trying it out, small installs)
#   --host HOST             create an Ingress for HOST, with TLS from --tls-secret NAME or --cluster-issuer NAME (cert-manager)
#   --ingress-class NAME    the Ingress class (default: the cluster's default)
#   --replicas N            control-plane replicas (default 2)
#   --browser-ttl MIN       a browser stops itself after MIN + 10 minutes (default 60)
#   --browser-image REF     (default ghcr.io/oyadotai/oya-browser:latest, pinned by digest)
#   --env-file FILE         optional settings (default: oya.env here if it exists); see ../ENVIRONMENT.md
#   --component DIR         an extra kustomize component (used by ../gcloud)
#   --sa-annotation K=V     an annotation for the control plane's ServiceAccount (e.g. Workload Identity)
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source-path=SCRIPTDIR source=../lib.sh
. "$HERE/../lib.sh"

CONTEXT='' NAMESPACE=oya

usage() { sed -n '2,29p' "$0" | sed 's/^# \{0,1\}//'; }

# kubectl against the named context and namespace.
k() { kubectl --context "$CONTEXT" -n "$NAMESPACE" "$@"; }

# One key of the oya-secrets Secret, or empty.
secret_key() { k get secret oya-secrets -o "jsonpath={.data.$1}" 2>/dev/null | base64 -d 2>/dev/null || true; }

require_context() {
  [ -n "$CONTEXT" ] || die "--context is required (kubectl config get-contexts lists them)"
  kubectl config get-contexts "$CONTEXT" >/dev/null 2>&1 || die "no kubectl context $CONTEXT"
}

# Builds and pushes the deployment image; prints it pinned by digest.
push_image() {
  local registry=$1 platform=$2 tag
  tag="${registry}:$(image_tag)"
  build_image "$tag" "$platform"
  log "Pushing $tag"
  docker push -q "$tag" >/dev/null
  pin_image "$tag"
}

# Creates oya-secrets on the first deploy only: a redeploy keeps it, or stored
# credentials become unreadable. A --database-url given later replaces just that key.
ensure_secrets() {
  local database_url=$1 in_cluster=$2 password
  if k get secret oya-secrets >/dev/null 2>&1; then
    [ -n "$database_url" ] && k patch secret oya-secrets -p "{\"stringData\":{\"DATABASE_URL\":$(jq -Rn --arg u "$database_url" '$u')}}" >/dev/null
    return 0
  fi
  password=$(random_hex 24)
  [ "$in_cluster" = 1 ] && database_url="postgres://oya:${password}@oya-postgres:5432/oya"
  [ -n "$database_url" ] || die "--database-url or --in-cluster-postgres is required on the first deploy"
  log "Creating the oya-secrets Secret (API key, profile and cluster secrets, database URL)"
  k create secret generic oya-secrets --from-literal=API_KEYS="$(random_hex 24)" \
    --from-literal=OYA_PROFILE_SECRET="$(random_hex 32)" --from-literal=OYA_CLUSTER_SECRET="$(random_hex 32)" \
    --from-literal=POSTGRES_PASSWORD="$password" --from-literal=DATABASE_URL="$database_url" >/dev/null
}

# Writes the optional settings into the oya-settings Secret (or removes it) and prints a version of them.
apply_settings() {
  local file=$1
  if [ -z "$file" ]; then
    k delete secret oya-settings --ignore-not-found >/dev/null
    echo none
    return
  fi
  [ -f "$file" ] || die "no settings file $file"
  log "Settings: $(grep -oE '^[A-Z_][A-Z0-9_]*' "$file" | paste -sd, - | sed 's/,/, /g')"
  k create secret generic oya-settings --from-env-file="$file" --dry-run=client -o yaml | k apply -f - >/dev/null
  openssl dgst -sha256 <"$file" | awk '{print substr($NF, 1, 16)}'
}

# Writes the overlay deploy.sh applies: namespace, images, replicas, config and optional components.
write_overlay() {
  local dir=$1 image=$2 browser=$3 replicas=$4 ttl=$5 settings=$6
  mkdir -p "$dir"
  {
    echo "# Written by deploy.sh; do not edit."
    echo "apiVersion: kustomize.config.k8s.io/v1beta1"
    echo "kind: Kustomization"
    echo "namespace: $NAMESPACE"
    echo "resources: [../../base]"
    [ ${#COMPONENTS[@]} -eq 0 ] || echo "components: [$(IFS=,; echo "${COMPONENTS[*]}")]"
    echo "images: [{ name: oya-control-plane, newName: ${image%@*}, digest: ${image#*@} }]"
    echo "replicas: [{ name: oya-control-plane, count: $replicas }]"
    echo "configMapGenerator:"
    echo "  - name: oya-config"
    echo "    literals:"
    echo "      - OYA_CLOUD_RUNTIME=k8s"
    echo "      - OYA_BROWSER_PROVIDER=oya-cloud"
    echo "      - OYA_CLOUD_IMAGE=$browser"
    echo "      - OYA_K8S_NAMESPACE=$NAMESPACE"
    echo "      - OYA_PUBLIC_WS_URL=ws://oya-control-plane.$NAMESPACE.svc.cluster.local:3100/ws"
    echo "      - OYA_CLOUD_SANDBOX_TTL_MINUTES=$ttl"
    echo "      - OYA_SETTINGS_VERSION=$settings"
    [ ${#PATCHES[@]} -eq 0 ] || { echo "patches:"; printf '%s\n' "${PATCHES[@]}"; }
  } >"$dir/kustomization.yaml"
}

# The Ingress patch for --host, as a JSON patch.
ingress_patch() {
  local ops
  ops=$(jq -cn --arg host "$1" --arg secret "${2:-oya-tls}" --arg issuer "$3" --arg class "$4" '
    [{op: "replace", path: "/spec/rules/0/host", value: $host},
     {op: "replace", path: "/spec/tls/0/hosts/0", value: $host},
     {op: "replace", path: "/spec/tls/0/secretName", value: $secret}]
    + (if $issuer != "" then [{op: "add", path: "/metadata/annotations/cert-manager.io~1cluster-issuer", value: $issuer}] else [] end)
    + (if $class != "" then [{op: "add", path: "/spec/ingressClassName", value: $class}] else [] end)')
  echo "  - target: { kind: Ingress, name: oya-control-plane }"
  echo "    patch: '$ops'"
}

# The ServiceAccount patch for --sa-annotation KEY=VALUE, as a JSON patch.
sa_patch() {
  local ops
  ops=$(jq -cn --arg k "${1%%=*}" --arg v "${1#*=}" '[{op: "add", path: "/metadata/annotations", value: {($k): $v}}]')
  echo "  - target: { kind: ServiceAccount, name: oya-control-plane }"
  echo "    patch: '$ops'"
}

cmd_deploy() {
  local registry='' image='' platform=linux/amd64 database_url='' in_cluster=0 host='' tls_secret='' issuer='' class=''
  local replicas=2 ttl=60 browser_image=ghcr.io/oyadotai/oya-browser:latest env_file='' browser settings overlay
  COMPONENTS=() PATCHES=()
  [ -f "$HERE/oya.env" ] && env_file="$HERE/oya.env"
  while [ $# -gt 0 ]; do
    case $1 in
      --context) CONTEXT=$2 && shift 2 ;;
      --namespace) NAMESPACE=$2 && shift 2 ;;
      --registry) registry=$2 && shift 2 ;;
      --image) image=$2 && shift 2 ;;
      --platform) platform=$2 && shift 2 ;;
      --database-url) database_url=$2 && shift 2 ;;
      --in-cluster-postgres) in_cluster=1 && shift ;;
      --host) host=$2 && shift 2 ;;
      --tls-secret) tls_secret=$2 && shift 2 ;;
      --cluster-issuer) issuer=$2 && shift 2 ;;
      --ingress-class) class=$2 && shift 2 ;;
      --replicas) replicas=$2 && shift 2 ;;
      --browser-ttl) ttl=$2 && shift 2 ;;
      --browser-image) browser_image=$2 && shift 2 ;;
      --env-file) env_file=$2 && shift 2 ;;
      --component) COMPONENTS+=("$(cd "$2" && pwd)") && shift 2 ;;
      --sa-annotation) PATCHES+=("$(sa_patch "$2")") && shift 2 ;;
      *) die "unknown option $1 (see ./deploy.sh)" ;;
    esac
  done
  need kubectl docker jq openssl git
  require_context
  [ -n "$image" ] || [ -n "$registry" ] || die "--registry (to build and push this checkout) or --image is required"
  [ -z "$host" ] || [ -n "$tls_secret" ] || [ -n "$issuer" ] || die "--host needs --tls-secret or --cluster-issuer"
  log "Deploying to context $CONTEXT, namespace $NAMESPACE"
  k create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl --context "$CONTEXT" apply -f - >/dev/null
  [ "$in_cluster" = 1 ] && COMPONENTS+=("$HERE/components/postgres")
  [ -n "$host" ] && COMPONENTS+=("$HERE/components/ingress") && PATCHES+=("$(ingress_patch "$host" "$tls_secret" "$issuer" "$class")")
  [ -n "$image" ] || image=$(push_image "$registry" "$platform")
  [[ $image == *@sha256:* ]] || image=$(pin_image "$image")
  browser=$(pin_image "$browser_image")
  ensure_secrets "$database_url" "$in_cluster"
  settings=$(apply_settings "$env_file")
  overlay="$HERE/.overlays/$CONTEXT-$NAMESPACE"
  # Components are referenced relative to the overlay, as kustomize requires.
  for i in "${!COMPONENTS[@]}"; do COMPONENTS[i]=$(python3 -c 'import os,sys; print(os.path.relpath(sys.argv[1], sys.argv[2]))' "${COMPONENTS[i]}" "$overlay"); done
  write_overlay "$overlay" "$image" "$browser" "$replicas" "$ttl" "$settings"
  log "Applying"
  kubectl --context "$CONTEXT" apply -k "$overlay" >/dev/null
  log "Waiting for the control plane to roll out"
  k rollout status deployment/oya-control-plane --timeout=15m
  [ "$in_cluster" = 1 ] && k rollout status statefulset/oya-postgres --timeout=5m
  log "Oya is up in $NAMESPACE${host:+ at https://$host}"
  log "API key: $(secret_key API_KEYS | cut -d, -f1)   (./deploy.sh keys --context $CONTEXT shows it again)"
}

# Runs the smoke test through a port-forward to the Service.
cmd_smoke() {
  local port=31${RANDOM:0:3} pid
  k port-forward svc/oya-control-plane "$port:3100" >/dev/null 2>&1 &
  pid=$!
  trap 'kill $pid 2>/dev/null' RETURN
  sleep 3
  smoke_test "http://127.0.0.1:$port" "$(secret_key API_KEYS | cut -d, -f1)"
}

cmd_status() {
  k get deployment,pods,svc,ingress -l 'app in (oya-control-plane, oya-postgres)' 2>/dev/null
  echo
  log "Browser pods:"
  k get pods -l oya-browser=true -L oya-owner 2>/dev/null
}

cmd_keys() { secret_key API_KEYS | tr ',' '\n'; }

cmd_destroy() {
  local data=0
  confirm "Remove Oya from $CONTEXT, namespace $NAMESPACE?"
  log "Stopping browser pods"
  k delete pods -l oya-browser=true --ignore-not-found --wait=false >/dev/null
  if [ -d "$HERE/.overlays/$CONTEXT-$NAMESPACE" ]; then
    kubectl --context "$CONTEXT" delete -k "$HERE/.overlays/$CONTEXT-$NAMESPACE" --ignore-not-found >/dev/null
    rm -rf "$HERE/.overlays/$CONTEXT-$NAMESPACE"
  fi
  [ "${DELETE_DATA:-0}" = 1 ] && data=1
  if [ "$data" = 1 ]; then
    confirm "Also delete the secrets and database volume? Stored credentials and data are gone for good."
    k delete secret oya-secrets oya-settings --ignore-not-found >/dev/null
    k delete pvc -l app=oya-postgres --ignore-not-found >/dev/null
    k delete pvc data-oya-postgres-0 --ignore-not-found >/dev/null
  else
    log "Kept the oya-secrets Secret and any database volume; --delete-data removes them"
  fi
  log "Removed"
}

# Options for the other commands: --context, --namespace, --delete-data, --yes.
parse_globals() {
  while [ $# -gt 0 ]; do
    case $1 in
      --context) CONTEXT=$2 && shift 2 ;;
      --namespace) NAMESPACE=$2 && shift 2 ;;
      --delete-data) export DELETE_DATA=1 && shift ;;
      --yes) export ASSUME_YES=1 && shift ;;
      *) die "unknown option $1" ;;
    esac
  done
  require_context
}

case ${1:-} in
  deploy) shift && cmd_deploy "$@" ;;
  smoke | status | keys | destroy) command=$1 && shift && parse_globals "$@" && "cmd_$command" ;;
  logs) shift && parse_globals "$@" && k logs -l app=oya-control-plane -f --max-log-requests 10 ;;
  *) usage && exit 1 ;;
esac
