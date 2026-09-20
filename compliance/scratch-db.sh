#!/usr/bin/env bash
# Brings up the throwaway Postgres the append-only check proves itself against,
# with the real migrations applied. Nothing here touches a live database: the
# point is that the proof runs anywhere, including CI, without prod access.
set -euo pipefail
NAME="${COMPLIANCE_PG_CONTAINER:-pg-audit}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

docker rm -f "$NAME" >/dev/null 2>&1 || true
docker run -d --name "$NAME" -e POSTGRES_PASSWORD=scratch postgres:16-alpine >/dev/null
until docker exec "$NAME" pg_isready -q 2>/dev/null; do sleep 1; done
docker exec "$NAME" psql -U postgres -q -c "create role service_role; create schema oya_browser;"

for m in 004_control_plane 012_audit_tamper_evidence; do
  docker cp "$ROOT/server/migrations/$m.sql" "$NAME:/tmp/$m.sql" >/dev/null
  docker exec "$NAME" psql -U postgres -q -v ON_ERROR_STOP=1 -f "/tmp/$m.sql" >/dev/null
done

# One chained row, so update and delete have something to be refused against.
docker exec "$NAME" psql -U postgres -q -c "insert into oya_browser.audit_log(action, actor, outcome, chain, seq, prev_hash, hash) values ('browser.provision','abcd','ok','scratch',1,repeat('0',64),'seed');"
echo "$NAME ready with 004 + 012 applied"
