-- Oya Browser schema
-- Run in Supabase SQL Editor (or via supabase db push)

-- ── Schema ──

create schema if not exists oya_browser;

-- ── User profiles (linked to Supabase Auth) ──

create table oya_browser.profiles (
  id uuid primary key references auth.users(id) on delete cascade,
  email text not null,
  display_name text,
  role text not null default 'user' check (role in ('user', 'admin')),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

-- ── API keys (owned by users) ──

create table oya_browser.api_keys (
  key text primary key,
  user_id uuid not null references oya_browser.profiles(id) on delete cascade,
  label text default 'Default',
  created_at timestamptz not null default now(),
  last_used_at timestamptz
);

create index idx_api_keys_user on oya_browser.api_keys(user_id);

-- ── Browser sessions (track connected browsers) ──

create table oya_browser.browsers (
  id text primary key,
  user_id uuid not null references oya_browser.profiles(id) on delete cascade,
  api_key text not null references oya_browser.api_keys(key) on delete cascade,
  name text not null default 'Browser',
  connected_at timestamptz not null default now(),
  last_seen_at timestamptz not null default now(),
  current_url text default ''
);

create index idx_browsers_user on oya_browser.browsers(user_id);
create index idx_browsers_api_key on oya_browser.browsers(api_key);

-- ── Auto-create profile on signup ──

create or replace function oya_browser.handle_new_user()
returns trigger as $$
begin
  insert into oya_browser.profiles (id, email, display_name)
  values (
    new.id,
    new.email,
    coalesce(new.raw_user_meta_data->>'display_name', split_part(new.email, '@', 1))
  );
  return new;
end;
$$ language plpgsql security definer;

create trigger on_auth_user_created
  after insert on auth.users
  for each row execute function oya_browser.handle_new_user();

-- ── Auto-update updated_at ──

create or replace function oya_browser.set_updated_at()
returns trigger as $$
begin
  new.updated_at = now();
  return new;
end;
$$ language plpgsql;

create trigger profiles_updated_at
  before update on oya_browser.profiles
  for each row execute function oya_browser.set_updated_at();

-- ── Row Level Security ──

alter table oya_browser.profiles enable row level security;
alter table oya_browser.api_keys enable row level security;
alter table oya_browser.browsers enable row level security;

-- Profiles: users see only their own
create policy "Users can read own profile"
  on oya_browser.profiles for select
  using (auth.uid() = id);

create policy "Users can update own profile"
  on oya_browser.profiles for update
  using (auth.uid() = id);

-- API keys: users manage only their own
create policy "Users can read own keys"
  on oya_browser.api_keys for select
  using (user_id = auth.uid());

create policy "Users can create own keys"
  on oya_browser.api_keys for insert
  with check (user_id = auth.uid());

create policy "Users can delete own keys"
  on oya_browser.api_keys for delete
  using (user_id = auth.uid());

-- Browsers: users see only their own
create policy "Users can read own browsers"
  on oya_browser.browsers for select
  using (user_id = auth.uid());

-- ── Service role access (server uses service key, bypasses RLS) ──
-- No extra policies needed — the service key skips RLS by default.

-- ── Grant schema access to authenticated users & service role ──

grant usage on schema oya_browser to authenticated, service_role;
grant all on all tables in schema oya_browser to service_role;
grant select, insert, update, delete on oya_browser.profiles to authenticated;
grant select, insert, delete on oya_browser.api_keys to authenticated;
grant select on oya_browser.browsers to authenticated;
