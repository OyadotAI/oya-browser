-- Keys an AI agent registered for itself (POST /auth/agent/signup), with no
-- account behind them until a person claims one by importing the key.
-- agent_email is the owner the agent named. A row with it and no user_id is an
-- unclaimed agent key: it runs the free desktop app and bring-your-own
-- browsers, but not the browsers this server runs (Oya Cloud), which cost money.
alter table oya_browser.api_keys add column if not exists agent_email text;
