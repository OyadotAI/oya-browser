-- Persona groups: a named set of one key's personas that a browser start can
-- name instead of one persona. Admission picks the least-loaded member, so a
-- caller spreads its starts across the set; the members stay ordinary personas,
-- each its own device, cookie jar and proxy.

create table if not exists oya_browser.persona_groups (
  owner text not null,              -- sha256 of the owning API key, never the key
  name text not null,               -- what the caller's code passes; unique per owner
  persona_ids jsonb not null default '[]'::jsonb,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  primary key (owner, name)
);

grant all on oya_browser.persona_groups to service_role;

-- Server-written through the service role, which bypasses RLS. No client
-- policy, as for personas: rows are keyed by an API-key fingerprint, so there
-- is no safe auth.uid() predicate. Reachable only through the API.
alter table oya_browser.persona_groups enable row level security;
