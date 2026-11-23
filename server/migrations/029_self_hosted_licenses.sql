-- Self-hosted licenses bought through Stripe. A self-hoster past the free cap
-- buys one on the console's billing page; each paid invoice issues a key,
-- recorded here beside the licenses admins issue. A bought key is kept sealed
-- by the server (AES-256-GCM, secrets.ts) so its buyer can see it again; an
-- admin's key is still never stored. One license per invoice: the unique index
-- makes a replayed webhook fail rather than issue a second key.

alter table oya_browser.licenses
  add column if not exists user_id text,
  add column if not exists stripe_customer_id text,
  add column if not exists stripe_subscription_id text,
  add column if not exists stripe_invoice_id text,
  add column if not exists key_sealed text,
  add column if not exists interval text;

create unique index if not exists licenses_stripe_invoice_id
  on oya_browser.licenses (stripe_invoice_id) where stripe_invoice_id is not null;
create index if not exists licenses_user_id on oya_browser.licenses (user_id);
