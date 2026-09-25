-- A project's routines: saved prompts its desktop apps run on a schedule, with
-- their run history. One row per routine, keyed by the project (the API key's
-- fingerprint) and the routine's id.
--
-- The value is sealed by the server (AES-256-GCM, secrets.ts): a prompt can say
-- what a person works on. version is checked and bumped by every write, so two
-- desktops claiming the same due run cannot both get it.

create table if not exists oya_browser.routines (
  owner text not null,
  id text not null,
  value text not null,
  version integer not null default 1,
  updated_at timestamptz not null default now(),
  primary key (owner, id)
);

grant all on oya_browser.routines to service_role;
alter table oya_browser.routines enable row level security;
