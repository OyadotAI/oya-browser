-- Control plane: usage accounting and the audit trail.
--
-- usage is bucketed by (api_key, hour) and written by the server on an
-- interval, never per event — at 1k-5k browsers a row per command would be
-- tens of thousands of inserts a second.
--
-- audit_log is append-only and records privileged, state-changing actions.
-- Actors are a sha256 fingerprint of the API key, never the key itself.

create table if not exists oya_browser.usage (
  api_key text not null,
  hour timestamptz not null,
  commands bigint not null default 0,
  command_errors bigint not null default 0,
  chat_requests bigint not null default 0,
  chat_input_tokens bigint not null default 0,
  chat_output_tokens bigint not null default 0,
  browser_seconds bigint not null default 0,
  browsers_started bigint not null default 0,
  cookie_pulls bigint not null default 0,
  frames bigint not null default 0,
  sandboxes_created bigint not null default 0,
  rate_limited bigint not null default 0,
  quota_denied bigint not null default 0,
  bytes_out bigint not null default 0,
  updated_at timestamptz not null default now(),
  primary key (api_key, hour)
);

create index if not exists idx_usage_hour on oya_browser.usage(hour desc);

create table if not exists oya_browser.audit_log (
  id bigserial primary key,
  ts timestamptz not null default now(),
  action text not null,
  actor text,                    -- sha256(api key) prefix, never the key
  actor_user uuid,
  target_type text,
  target_id text,
  outcome text not null default 'ok',
  ip text,
  user_agent text,
  meta jsonb
);

create index if not exists idx_audit_ts on oya_browser.audit_log(ts desc);
create index if not exists idx_audit_action on oya_browser.audit_log(action, ts desc);
create index if not exists idx_audit_actor on oya_browser.audit_log(actor, ts desc);

grant all on oya_browser.usage to service_role;
grant all on oya_browser.audit_log to service_role;
grant usage, select on sequence oya_browser.audit_log_id_seq to service_role;

-- ── Row level security ──
--
-- Both tables are server-written through the service role, which bypasses RLS.
-- No client-facing policy is granted: usage is keyed by API key rather than
-- user id so there is no safe auth.uid() predicate, and an audit trail a
-- tenant could read would leak other tenants' activity. Both are exposed
-- through the admin API instead.

alter table oya_browser.usage enable row level security;
alter table oya_browser.audit_log enable row level security;
