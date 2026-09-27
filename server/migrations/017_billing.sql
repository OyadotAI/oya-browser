-- Billing for the hosted cloud. Usage gains the counters plans are priced on:
-- seconds of browsers this server runs (cloud_seconds, apart from desktop
-- browser_seconds), what the operator's LLM key cost in micro-USD, and agent
-- steps. The same usage table also holds one row per person, api_key =
-- 'u:<user id>', so a month's usage survives a deleted key.
--
-- subscriptions is a person's plan as Stripe last said it, and what of the
-- period's usage has already been reported to Stripe's meters.

alter table oya_browser.usage add column if not exists cloud_seconds bigint not null default 0;
alter table oya_browser.usage add column if not exists hosted_llm_microusd bigint not null default 0;
alter table oya_browser.usage add column if not exists agent_steps bigint not null default 0;

create table if not exists oya_browser.subscriptions (
  user_id text primary key,
  stripe_customer_id text,
  stripe_subscription_id text,
  plan text not null default 'free',
  status text,
  period_start timestamptz,
  period_end timestamptz,
  reported jsonb,
  stripe_event_at bigint,
  updated_at timestamptz not null default now()
);

grant all on oya_browser.subscriptions to service_role;
alter table oya_browser.subscriptions enable row level security;
