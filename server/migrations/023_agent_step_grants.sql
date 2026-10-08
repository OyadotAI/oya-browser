/** Support grants extend agent allowance without rewriting metered usage. */
alter table oya_browser.billing_grants add column if not exists agent_steps bigint not null default 0 check (agent_steps >= 0);
