/** Administrator adjustments never overwrite Stripe's subscription state. */
create table if not exists oya_browser.billing_overrides (
  user_id text primary key,
  plan text check (plan in ('free', 'developer', 'startup')),
  actor text not null,
  reason text not null,
  updated_at timestamptz not null
);

-- Immutable grants expire with their billing period. UUID request ids make retries safe.
create table if not exists oya_browser.billing_grants (
  id text primary key,
  user_id text not null,
  period_start timestamptz not null,
  cloud_seconds bigint not null check (cloud_seconds >= 0),
  hosted_llm_microusd bigint not null check (hosted_llm_microusd >= 0),
  actor text not null,
  reason text not null,
  created_at timestamptz not null
);
create index if not exists billing_grants_period on oya_browser.billing_grants (user_id, period_start);
grant all on oya_browser.billing_overrides, oya_browser.billing_grants to service_role;
alter table oya_browser.billing_overrides enable row level security;
alter table oya_browser.billing_grants enable row level security;
