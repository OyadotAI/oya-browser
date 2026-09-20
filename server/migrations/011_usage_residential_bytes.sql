-- Residential proxy egress is metered per key like every other usage counter
-- (server/src/usage.js, server/src/ws-handler.js), but the column was never
-- added here. The write failed the whole row, so every deployment on Postgres
-- silently fell back to the local usage.json spool and stopped accounting
-- across replicas. Backfills as zero, which is what an unmetered hour was.
alter table oya_browser.usage add column if not exists residential_proxy_bytes bigint not null default 0;
