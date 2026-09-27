-- What the admin page reads that was only ever sent to PostHog: self-hosted
-- installs as their daily ping last described them, and downloads counted per
-- day. And the self-hosted licenses admins issue, so they can be listed and
-- revoked; the license key itself is shown once and never stored.

create table if not exists oya_browser.installs (
  install_id text primary key,
  version text,
  browsers integer not null default 0,
  peak_cloud integer not null default 0,
  license_id text,
  pings integer not null default 0,
  first_seen timestamptz not null default now(),
  last_seen timestamptz not null default now()
);

create table if not exists oya_browser.download_counts (
  day text not null,
  kind text not null,
  platform text not null,
  count bigint not null default 0,
  primary key (day, kind, platform)
);

create table if not exists oya_browser.licenses (
  id text primary key,
  licensee text not null,
  max_concurrent integer not null,
  expires_at timestamptz not null,
  created_at timestamptz not null default now(),
  created_by text,
  revoked_at timestamptz
);

grant all on oya_browser.installs, oya_browser.download_counts, oya_browser.licenses to service_role;
alter table oya_browser.installs enable row level security;
alter table oya_browser.download_counts enable row level security;
alter table oya_browser.licenses enable row level security;
