-- Personas: one identity = fingerprint + cookie jar + proxy, bound together.
--
-- The seed, not the fingerprint, is stored: the fingerprint is derived from it
-- deterministically, so a persona's device stays byte-identical for its whole
-- life. That stability is what keeps the fingerprint coherent with the cookies
-- it is paired with — a returning session that looks like a new device is the
-- signal this model exists to avoid.

create table if not exists oya_browser.personas (
  id text primary key,
  owner text not null,              -- sha256 of the owning API key, never the key
  name text not null,
  seed bigint not null,
  proxy jsonb,
  max_concurrent integer,           -- null = uncapped (the per-key default persona)
  is_default boolean not null default false,
  created_at timestamptz not null default now(),
  last_used_at timestamptz,
  updated_at timestamptz not null default now()
);

create index if not exists idx_personas_owner on oya_browser.personas(owner);

grant all on oya_browser.personas to service_role;

-- Server-written through the service role, which bypasses RLS. No client
-- policy: personas are keyed by an API-key fingerprint, so there is no safe
-- auth.uid() predicate, and one tenant listing another's personas would expose
-- how their fleet is shaped. Reachable only through the API.
alter table oya_browser.personas enable row level security;
