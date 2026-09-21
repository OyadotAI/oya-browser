-- Per-account chat settings — each user gets their own OpenAI key, model, and
-- base URL. The existing oya_browser.settings table stays as the server-wide
-- default, so resolution is: user_settings -> settings -> environment.

create table oya_browser.user_settings (
  user_id uuid not null references oya_browser.profiles(id) on delete cascade,
  key text not null,
  value text not null,
  updated_at timestamptz not null default now(),
  primary key (user_id, key)
);

create index idx_user_settings_user on oya_browser.user_settings(user_id);

-- Grant access
grant all on oya_browser.user_settings to service_role;

-- ── Row level security ──

alter table oya_browser.user_settings enable row level security;

-- Users see and manage only their own settings. The server uses the service
-- role and bypasses this; the policy protects direct client access.
create policy "Users can read own settings"
  on oya_browser.user_settings for select
  using (auth.uid() = user_id);

create policy "Users can write own settings"
  on oya_browser.user_settings for all
  using (auth.uid() = user_id);
