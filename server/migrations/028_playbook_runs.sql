-- A playbook's run history: its newest finished runs, so every replica, the
-- console and the desktop app can show how a playbook last ran after the
-- in-memory run is gone. One row per project (the API key's fingerprint) and
-- playbook name.
--
-- The value is sealed by the server (AES-256-GCM, secrets.ts): a failed run's
-- error can carry page text. version is checked and bumped by every write, so
-- two replicas finishing runs at once both land.

create table if not exists oya_browser.playbook_runs (
  owner text not null,
  playbook text not null,
  value text not null,
  version integer not null default 1,
  updated_at timestamptz not null default now(),
  primary key (owner, playbook)
);

grant all on oya_browser.playbook_runs to service_role;
alter table oya_browser.playbook_runs enable row level security;
