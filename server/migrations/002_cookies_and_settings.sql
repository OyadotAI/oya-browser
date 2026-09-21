-- Cookies and settings tables — move from file-based to DB

-- ── Shared cookie jar ──

create table oya_browser.cookies (
  id text primary key,            -- "domain|path|name"
  data jsonb not null,
  updated_at timestamptz not null default now()
);

-- ── Server settings (key-value) ──

create table oya_browser.settings (
  key text primary key,
  value text not null,
  updated_at timestamptz not null default now()
);

-- Grant access
grant all on oya_browser.cookies to service_role;
grant all on oya_browser.settings to service_role;
