-- Sealed records that used to live only in the data directory: a persona's
-- site credentials (credentials.json), its MFA factors (mfa.json) and its login
-- state, cookies and localStorage (cookies.json). On a Postgres deployment they
-- now live here, so they survive a redeploy and every replica sees them.
--
-- Values are sealed by the server (AES-256-GCM, secrets.ts) before they are
-- written; the database never holds a password, seed or cookie in the clear.

create table if not exists oya_browser.persona_credentials (
  id text primary key,              -- persona id | site
  value text not null,
  updated_at timestamptz not null default now()
);

create table if not exists oya_browser.mfa_factors (
  id text primary key,              -- persona id, or persona id | site
  value text not null,
  updated_at timestamptz not null default now()
);

create table if not exists oya_browser.persona_logins (
  id text primary key,              -- persona id
  value text not null,
  updated_at timestamptz not null default now()
);

grant all on oya_browser.persona_credentials to service_role;
grant all on oya_browser.mfa_factors to service_role;
grant all on oya_browser.persona_logins to service_role;

-- Server-written only; no client policy, as with personas.
alter table oya_browser.persona_credentials enable row level security;
alter table oya_browser.mfa_factors enable row level security;
alter table oya_browser.persona_logins enable row level security;
