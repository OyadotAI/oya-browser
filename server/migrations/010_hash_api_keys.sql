-- api_keys stored the bearer credential itself, so any read-only exposure of
-- that table — a backup, a replica, a support export, an over-broad grant —
-- handed over working administrator credentials for every tenant's browsers,
-- cookie jars, personas and recordings. Everything else in this control plane
-- already records a SHA-256 digest instead (audit.js, key-config.js,
-- personas.owner, sandbox owner labels).
--
-- After this, the table holds sha256(key), an 8-character prefix for display,
-- and the project id the key opens. A key is shown to its owner exactly once,
-- when it is created; the console opens a project with a scoped, expiring
-- credential minted by POST /auth/projects/:id/access.
--
-- ORDER MATTERS: apply this before deploying the code that reads key_hash, and
-- do not run the old code against the new schema — it selects a column that is
-- gone. On the SQLite and plain-Postgres paths api_keys does not exist at all,
-- which is why every statement is guarded.
--
-- sha256() is built into Postgres 11+; no pgcrypto needed.

do $$
begin
  if to_regclass('oya_browser.api_keys') is null then
    raise notice 'oya_browser.api_keys does not exist on this deployment; nothing to migrate';
    return;
  end if;

  alter table oya_browser.api_keys add column if not exists key_hash text;
  alter table oya_browser.api_keys add column if not exists key_prefix text;
  alter table oya_browser.api_keys add column if not exists project text;

  -- Nothing to backfill if `key` is already gone (re-running this migration).
  if exists (select 1 from information_schema.columns
             where table_schema = 'oya_browser' and table_name = 'api_keys' and column_name = 'key') then

    execute $backfill$
      update oya_browser.api_keys
         set key_hash   = encode(sha256(convert_to(key, 'UTF8')), 'hex'),
             key_prefix = left(key, 8),
             project    = 'prj_' || left(encode(sha256(convert_to(key, 'UTF8')), 'hex'), 24)
       where key_hash is null
    $backfill$;

    -- browsers.api_key referenced the plaintext column. That table is legacy
    -- inventory, read once by control/migrate.js; the reference has to go
    -- before the column can.
    alter table oya_browser.browsers drop constraint if exists browsers_api_key_fkey;

    alter table oya_browser.api_keys alter column key_hash set not null;
    alter table oya_browser.api_keys drop constraint if exists api_keys_pkey;
    alter table oya_browser.api_keys add primary key (key_hash);
    alter table oya_browser.api_keys drop column key;
  end if;

  create index if not exists idx_api_keys_project on oya_browser.api_keys(project);

  -- usage rows were keyed by the plaintext key too — the same credential, one
  -- table over. They become the same 16-character fingerprint audit.js uses.
  if to_regclass('oya_browser.usage') is not null then
    execute $usage$
      update oya_browser.usage
         set api_key = left(encode(sha256(convert_to(api_key, 'UTF8')), 'hex'), 16)
       where api_key !~ '^[0-9a-f]{16}$'
    $usage$;
  end if;

  -- Nothing reaches api_keys except the server, which uses the service role.
  -- The grants and policies from 001 described an access path the console has
  -- never used, and an INSERT one would have been a way to register a key of
  -- the user's own choosing straight past the API's checks.
  revoke select, insert, update, delete on oya_browser.api_keys from authenticated;
  drop policy if exists "Users can read own keys" on oya_browser.api_keys;
  drop policy if exists "Users can create own keys" on oya_browser.api_keys;
  drop policy if exists "Users can delete own keys" on oya_browser.api_keys;
end $$;
