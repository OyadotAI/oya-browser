-- api_keys on plain Postgres: the digests of stored API keys, and who owns each.
--
-- On Supabase this table arrives through 001 (created against auth.users) and
-- 010 (which replaced the plaintext key with its digest). Neither can run here,
-- so this creates it in the shape those two leave behind. user_id has no
-- foreign key: accounts live in Supabase Auth, and a key with no account (an
-- operator's, or an agent's unclaimed one) has none. 014 then adds agent_email.

create table if not exists oya_browser.api_keys (
  key_hash text primary key,       -- sha256 of the key, never the key
  key_prefix text,
  project text,
  user_id uuid,
  label text default 'Default',
  created_at timestamptz not null default now(),
  last_used_at timestamptz
);

create index if not exists idx_api_keys_user on oya_browser.api_keys(user_id);
create index if not exists idx_api_keys_project on oya_browser.api_keys(project);

grant select, insert, update, delete on oya_browser.api_keys to service_role;
