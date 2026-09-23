#!/usr/bin/env bash
# Deploy Oya to Google Cloud: GKE Autopilot runs the control plane and one pod
# per browser, Cloud SQL (private IP) holds its state, and access is through
# Kubernetes RBAC plus a Google service account bound by Workload Identity.
# Everything on the cluster is ../k8s, applied with a GKE component. See README.md.
#
#   ./deploy.sh deploy --project ID [--domain oya.example.com] [options]
#   ./deploy.sh smoke | status | keys | logs   --project ID
#   ./deploy.sh destroy --project ID [--yes]
#
# --project is always required, so nothing lands in whichever project gcloud has active.
#
# Options:
#   --region REGION         (default us-central1)
#   --name NAME             cluster, database and registry name (default oya)
#   --domain DOMAIN         HTTPS on a reserved IP with a Google-managed certificate; point DOMAIN's A record at the IP printed
#   --replicas N            control-plane replicas (default 2)
#   --db-tier TIER          Cloud SQL machine (default db-custom-1-3840)
#   --single-zone-db        no standby database (cheaper, not for production)
#   --browser-image REF     (default ghcr.io/oyadotai/oya-browser:latest, pinned by digest)
#   --env-file FILE         optional settings (default: oya.env here if it exists); see ../ENVIRONMENT.md
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source-path=SCRIPTDIR source=../lib.sh
. "$HERE/../lib.sh"

PROJECT='' REGION=us-central1 NAME=oya NAMESPACE=oya

usage() { sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'; }

# gcloud in the named project.
g() { gcloud --project "$PROJECT" --quiet "$@"; }

context() { echo "gke_${PROJECT}_${REGION}_${NAME}"; }
k8s() { "$HERE/../k8s/deploy.sh" "$@" --context "$(context)" --namespace "$NAMESPACE"; }
service_account() { echo "$NAME-control-plane@$PROJECT.iam.gserviceaccount.com"; }

require_project() {
  [ -n "$PROJECT" ] || die "--project is required"
  g projects describe "$PROJECT" >/dev/null || die "no access to project $PROJECT"
}

enable_apis() {
  log "Enabling the APIs Oya uses"
  g services enable container.googleapis.com sqladmin.googleapis.com artifactregistry.googleapis.com \
    servicenetworking.googleapis.com compute.googleapis.com iamcredentials.googleapis.com
}

# Private services access, so Cloud SQL has only a private IP in the default network.
ensure_private_services() {
  g compute addresses describe "$NAME-sql-range" --global >/dev/null 2>&1 ||
    g compute addresses create "$NAME-sql-range" --global --purpose=VPC_PEERING --prefix-length=20 --network=default
  g services vpc-peerings connect --service=servicenetworking.googleapis.com --ranges="$NAME-sql-range" --network=default 2>/dev/null ||
    g services vpc-peerings update --service=servicenetworking.googleapis.com --ranges="$NAME-sql-range" --network=default --force
}

# Creates the database once; prints the database URL (through the proxy) only when it created the user.
ensure_database() {
  local availability=$1 tier=$2 password
  if ! g sql instances describe "$NAME-db" >/dev/null 2>&1; then
    log "Creating Cloud SQL $NAME-db ($availability); about 10 minutes"
    g sql instances create "$NAME-db" --database-version=POSTGRES_16 --tier="$tier" --region="$REGION" \
      --network=default --no-assign-ip --availability-type="$availability" --storage-auto-increase \
      --backup-start-time=03:00 --enable-point-in-time-recovery --deletion-protection >&2
    g sql databases create oya --instance="$NAME-db" >&2
  fi
  g sql users list --instance="$NAME-db" --format='value(name)' | grep -qx oya && return
  password=$(random_hex 24)
  g sql users create oya --instance="$NAME-db" --password="$password" >&2
  echo "postgres://oya:${password}@127.0.0.1:5432/oya"
}

ensure_cluster() {
  if ! g container clusters describe "$NAME" --region "$REGION" >/dev/null 2>&1; then
    log "Creating GKE Autopilot cluster $NAME; about 10 minutes"
    g container clusters create-auto "$NAME" --region "$REGION" --network default --release-channel regular
  fi
  g container clusters get-credentials "$NAME" --region "$REGION" >/dev/null
}

# The Google service account the control plane runs as: Cloud SQL client, nothing else.
ensure_identity() {
  local sa
  sa=$(service_account)
  g iam service-accounts describe "$sa" >/dev/null 2>&1 ||
    g iam service-accounts create "$NAME-control-plane" --display-name "Oya control plane" >/dev/null
  g projects add-iam-policy-binding "$PROJECT" --member "serviceAccount:$sa" --role roles/cloudsql.client --condition=None >/dev/null
  g iam service-accounts add-iam-policy-binding "$sa" --role roles/iam.workloadIdentityUser \
    --member "serviceAccount:$PROJECT.svc.id.goog[$NAMESPACE/oya-control-plane]" >/dev/null
}

# The registry the image is pushed to, and docker signed in to it.
ensure_registry() {
  g artifacts repositories describe "$NAME" --location "$REGION" >/dev/null 2>&1 ||
    g artifacts repositories create "$NAME" --location "$REGION" --repository-format docker >&2
  gcloud auth configure-docker "$REGION-docker.pkg.dev" --quiet >/dev/null 2>&1
  echo "$REGION-docker.pkg.dev/$PROJECT/$NAME/control-plane"
}

# The GKE component for this deployment: the SQL proxy, the load balancer settings, and the HTTPS Ingress for --domain.
write_component() {
  local dir=$1 domain=$2 instance
  instance=$(g sql instances describe "$NAME-db" --format='value(connectionName)')
  rm -rf "$dir" && mkdir -p "$dir"
  sed "s|INSTANCE|$instance|" "$HERE/gke/sql-proxy.yaml" >"$dir/sql-proxy.yaml"
  cp "$HERE/gke/backend-config.yaml" "$HERE/gke/service-patch.yaml" "$dir/"
  [ -n "$domain" ] && sed "s|DOMAIN|$domain|; s|ADDRESS|$NAME-ip|" "$HERE/gke/ingress.yaml" >"$dir/ingress.yaml"
  {
    echo "# Written by deploy.sh; do not edit."
    echo "apiVersion: kustomize.config.k8s.io/v1alpha1"
    echo "kind: Component"
    echo "resources: [backend-config.yaml${domain:+, ingress.yaml}]"
    echo "patches: [{ path: sql-proxy.yaml }, { path: service-patch.yaml }]"
  } >"$dir/kustomization.yaml"
}

# A reserved IP and a TLS 1.2+ policy for the HTTPS load balancer.
ensure_https() {
  g compute addresses describe "$NAME-ip" --global >/dev/null 2>&1 || g compute addresses create "$NAME-ip" --global
  g compute ssl-policies describe oya-tls >/dev/null 2>&1 ||
    g compute ssl-policies create oya-tls --profile MODERN --min-tls-version 1.2 >/dev/null
}

cmd_deploy() {
  local domain='' replicas=2 tier=db-custom-1-3840 availability=REGIONAL browser_image=ghcr.io/oyadotai/oya-browser:latest
  local env_file='' registry database_url component extra=()
  [ -f "$HERE/oya.env" ] && env_file="$HERE/oya.env"
  while [ $# -gt 0 ]; do
    case $1 in
      --project) PROJECT=$2 && shift 2 ;;
      --region) REGION=$2 && shift 2 ;;
      --name) NAME=$2 && shift 2 ;;
      --domain) domain=$2 && shift 2 ;;
      --replicas) replicas=$2 && shift 2 ;;
      --db-tier) tier=$2 && shift 2 ;;
      --single-zone-db) availability=ZONAL && shift ;;
      --browser-image) browser_image=$2 && shift 2 ;;
      --env-file) env_file=$2 && shift 2 ;;
      *) die "unknown option $1 (see ./deploy.sh)" ;;
    esac
  done
  need gcloud kubectl docker jq openssl git python3
  require_project
  [ -n "$domain" ] || warn "no --domain: no public endpoint; reach it with kubectl port-forward (./deploy.sh smoke does)"
  log "Deploying to project $PROJECT, region $REGION"
  enable_apis
  ensure_private_services
  registry=$(ensure_registry)
  ensure_cluster
  ensure_identity
  database_url=$(ensure_database "$availability" "$tier")
  [ -n "$domain" ] && ensure_https
  component="$HERE/.generated/$PROJECT-$NAME"
  write_component "$component" "$domain"
  [ -n "$database_url" ] && extra+=(--database-url "$database_url")
  [ -n "$env_file" ] && extra+=(--env-file "$env_file")
  k8s deploy --registry "$registry" --platform linux/amd64 --replicas "$replicas" --browser-image "$browser_image" \
    --component "$component" --sa-annotation "iam.gke.io/gcp-service-account=$(service_account)" "${extra[@]}"
  [ -n "$domain" ] && log "Point $domain's A record at $(g compute addresses describe "$NAME-ip" --global --format='value(address)'); the certificate is issued once DNS resolves (up to an hour)"
  return 0
}

cmd_destroy() {
  confirm "Delete Oya from project $PROJECT: the cluster, the database (and its backups), the registry and the service account?"
  if g container clusters describe "$NAME" --region "$REGION" >/dev/null 2>&1; then
    g container clusters get-credentials "$NAME" --region "$REGION" >/dev/null
    ASSUME_YES=1 k8s destroy --delete-data || true
    log "Deleting the cluster"
    g container clusters delete "$NAME" --region "$REGION"
  fi
  if g sql instances describe "$NAME-db" >/dev/null 2>&1; then
    log "Deleting the database"
    g sql instances patch "$NAME-db" --no-deletion-protection >/dev/null
    g sql instances delete "$NAME-db"
  fi
  g artifacts repositories delete "$NAME" --location "$REGION" 2>/dev/null || true
  g iam service-accounts delete "$(service_account)" 2>/dev/null || true
  g compute addresses delete "$NAME-ip" --global 2>/dev/null || true
  g compute ssl-policies delete oya-tls 2>/dev/null || true
  g services vpc-peerings delete --service=servicenetworking.googleapis.com --network=default 2>/dev/null || true
  g compute addresses delete "$NAME-sql-range" --global 2>/dev/null || true
  rm -rf "$HERE/.generated/$PROJECT-$NAME"
  log "Removed"
}

# Options for the other commands.
parse_globals() {
  while [ $# -gt 0 ]; do
    case $1 in
      --project) PROJECT=$2 && shift 2 ;;
      --region) REGION=$2 && shift 2 ;;
      --name) NAME=$2 && shift 2 ;;
      --yes) export ASSUME_YES=1 && shift ;;
      *) die "unknown option $1" ;;
    esac
  done
  require_project
}

case ${1:-} in
  deploy) shift && cmd_deploy "$@" ;;
  destroy) shift && parse_globals "$@" && cmd_destroy ;;
  smoke | status | keys | logs) command=$1 && shift && parse_globals "$@" && k8s "$command" ;;
  *) usage && exit 1 ;;
esac
