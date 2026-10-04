-- API keys can expire. expires_at is when a key stops authenticating; null
-- means never, which is every key made before this and every key made without
-- an expiry since. The console offers 30, 90 or 365 days, and an operator can
-- cap new keys with OYA_API_KEY_MAX_DAYS. A refused expired key is audited as
-- key.expired.
--
-- Project credentials (oya_ tokens in the control plane) already carry their
-- own expiresAt and are not touched here.
--
-- ORDER MATTERS: apply this before deploying the code that writes expires_at,
-- or creating and importing keys fails on the missing column. SQLite adds the
-- column itself when it opens its file.

do $$
begin
  if to_regclass('oya_browser.api_keys') is null then
    raise notice 'oya_browser.api_keys does not exist on this deployment; nothing to migrate';
    return;
  end if;

  alter table oya_browser.api_keys add column if not exists expires_at timestamptz;
  comment on column oya_browser.api_keys.expires_at is 'When the key stops authenticating; null means never.';
end $$;
