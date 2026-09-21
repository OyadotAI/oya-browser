-- Settings, keyed by API key.
--
-- The API key is the identity for browsers, personas, cookies and usage, so it
-- is the identity for configuration too. `owner` is a sha256 fingerprint of the
-- key, never the key itself — the same convention as personas and audit_log.
--
-- Credential values (LLM key, provider keys, solver key) are sealed with the
-- envelope scheme in secrets.js before they reach this table; the rest are
-- stored as written.

create table if not exists oya_browser.key_settings (
  owner text not null,
  key text not null,
  value text not null,
  updated_at timestamptz not null default now(),
  primary key (owner, key)
);

create index if not exists idx_key_settings_owner on oya_browser.key_settings(owner);

grant all on oya_browser.key_settings to service_role;

-- Server-written through the service role, which bypasses RLS. No client
-- policy: rows are keyed by an API-key fingerprint, so there is no safe
-- auth.uid() predicate. Reachable only through the API.
alter table oya_browser.key_settings enable row level security;

-- The per-account layer this replaces. Settings now follow the API key, so
-- these rows are no longer read. Drop them once you have confirmed nothing
-- depends on them:
--   drop table if exists oya_browser.user_settings;
