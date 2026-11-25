-- Extend anonymous Keel usage without inventing measurements for older reports.
alter table oya_browser.keel_usage add column if not exists runtime_ms bigint;
alter table oya_browser.keel_usage add column if not exists project_ids jsonb;

create table if not exists oya_browser.keel_installs (
  install_id text primary key,
  first_seen timestamptz not null,
  last_seen timestamptz not null,
  version text not null,
  os text not null,
  license_id text
);
grant all on oya_browser.keel_installs to service_role;
alter table oya_browser.keel_installs enable row level security;

-- Original receipt times define presence; applying this migration creates no new activity.
insert into oya_browser.keel_installs (install_id, first_seen, last_seen, version, os, license_id)
select distinct on (install_id)
  install_id, min(received_at) over (partition by install_id), received_at, version, os, license_id
from oya_browser.keel_usage
order by install_id, received_at desc, day desc
on conflict (install_id) do nothing;

create index if not exists keel_usage_day on oya_browser.keel_usage (day);
create index if not exists keel_installs_last_seen on oya_browser.keel_installs (last_seen desc);
