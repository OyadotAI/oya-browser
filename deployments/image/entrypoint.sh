#!/bin/sh
# Starts the Oya server the way every deployment in deployments/ runs it:
# find the address other replicas reach this one at, apply the database
# schema, then hand the process over to the server.
set -eu

# Optional settings (see deployments/ENVIRONMENT.md) arrive as one JSON object
# where a platform cannot pass a variable list, as on ECS where they are one
# Secrets Manager secret. Anything the deployment sets itself wins.
if [ -n "${OYA_SETTINGS_JSON:-}" ]; then
  eval "$(node -e '
    const q = "\x27";
    for (const [key, value] of Object.entries(JSON.parse(process.env.OYA_SETTINGS_JSON))) {
      if (!/^[A-Z_][A-Z0-9_]*$/.test(key) || process.env[key] !== undefined) continue;
      console.log("export " + key + "=" + q + String(value).split(q).join(q + "\\" + q + q) + q);
    }')"
  unset OYA_SETTINGS_JSON
fi

# Replicas route requests for a browser to the replica that holds it, so each
# one announces its own address. Kubernetes passes the pod IP in POD_IP; on
# ECS the task metadata endpoint has it.
if [ -z "${OYA_INSTANCE_URL:-}" ] && [ -n "${POD_IP:-}" ]; then
  export OYA_INSTANCE_URL="http://${POD_IP}:${PORT:-3100}"
fi
if [ -z "${OYA_INSTANCE_URL:-}" ] && [ -n "${ECS_CONTAINER_METADATA_URI_V4:-}" ]; then
  ip=$(node -e 'fetch(process.env.ECS_CONTAINER_METADATA_URI_V4).then((r) => r.json()).then((m) => console.log(m.Networks[0].IPv4Addresses[0]))')
  export OYA_INSTANCE_URL="http://${ip}:${PORT:-3100}"
fi

# Migrations are idempotent and recorded per file. Replicas that start
# together race to apply the same file; the loser fails, waits for the
# winner to commit, and then finds nothing left to do.
if [ -n "${DATABASE_URL:-}" ]; then
  attempt=0
  until node migrations/run.mjs; do
    attempt=$((attempt + 1))
    [ "$attempt" -ge 5 ] && echo "[oya] migrations failed $attempt times, giving up" >&2 && exit 1
    sleep $((attempt * 3))
  done
fi

exec node src/index.ts
