#!/usr/bin/env bash
# Applies SQL migrations from inside the cluster, so a database on a private IP
# (Cloud SQL behind private services access) is reachable: a GitHub runner is
# not on that network and its psql would time out.
#
# usage: migrate-in-cluster.sh <namespace> <name> <database-url> <file.sql>...
#
# The files run in the order given, in one transaction with ON_ERROR_STOP, the
# same as the runner-side psql this replaces. The URL travels in a Secret, never
# in the Job spec. The Job, its ConfigMap and Secret are removed afterwards
# whether it passed or not, and the step fails when the migration does.
set -euo pipefail

namespace=$1 name=$2 url=$3
shift 3
: "${url:?Missing database connection for the ${name} migrations}"
(($#)) || { echo "no migration files given" >&2; exit 2; }

job="migrate-${name}"
deadline_seconds=300
image=postgres:17-alpine
postgres_uid=70

# Removes this migration's Job, ConfigMap and Secret; `--wait=true` before a run so a leftover cannot collide.
cleanup() {
  kubectl delete "job/$job" "configmap/$job" "secret/$job" -n "$namespace" --ignore-not-found "--wait=${1:-false}" >/dev/null
}
cleanup true
trap cleanup EXIT

from_files=() psql_files=()
for file in "$@"; do
  from_files+=("--from-file=$(basename "$file")=$file")
  psql_files+=("\"--file=/migrations/$(basename "$file")\"")
done
file_args=$(IFS=,; echo "${psql_files[*]}")
kubectl create configmap "$job" -n "$namespace" "${from_files[@]}" >/dev/null
kubectl create secret generic "$job" -n "$namespace" --from-literal=PGDATABASE="$url" >/dev/null

kubectl apply -n "$namespace" -f - >/dev/null <<YAML
apiVersion: batch/v1
kind: Job
metadata:
  name: $job
spec:
  backoffLimit: 0
  activeDeadlineSeconds: $deadline_seconds
  template:
    spec:
      restartPolicy: Never
      securityContext:
        runAsNonRoot: true
        runAsUser: $postgres_uid
      containers:
        - name: psql
          image: $image
          command: ["psql", "-X", "--dbname=\$(PGDATABASE)", "--set=ON_ERROR_STOP=1", "--single-transaction", $file_args]
          env:
            - name: PGCONNECT_TIMEOUT
              value: "10"
            - name: PGDATABASE
              valueFrom:
                secretKeyRef: { name: $job, key: PGDATABASE }
          volumeMounts:
            - { name: migrations, mountPath: /migrations, readOnly: true }
      volumes:
        - name: migrations
          configMap: { name: $job }
YAML

# Wait for either outcome: `kubectl wait` takes one condition, and waiting only
# for Complete would sit out the whole deadline on a failed migration.
status=
for ((waited = 0; waited <= deadline_seconds; waited += 5)); do
  status=$(kubectl get job "$job" -n "$namespace" -o jsonpath='{.status.succeeded}/{.status.failed}')
  [[ $status == 1/* || $status == */1 ]] && break
  sleep 5
done
kubectl logs "job/$job" -n "$namespace" --all-containers || true
if [[ $status != 1/* ]]; then
  # A pod that never started (unschedulable, image pull, admission) has no logs; say why before cleanup removes it.
  kubectl describe "job/$job" -n "$namespace" || true
  kubectl get events -n "$namespace" --field-selector "involvedObject.name=$job" --sort-by=.lastTimestamp || true
  echo "::error::${name} migrations failed (job status ${status:-unknown})"
  exit 1
fi
echo "${name} migrations applied"
