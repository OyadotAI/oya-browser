-- Preserve AWS buyer ownership and pending registration separately from Stripe
-- subscriptions. These rows never establish active service access. No cascading
-- auth-user foreign key: deletion must not make a paid purchase claimable again.

create table if not exists oya_browser.aws_marketplace_accounts (
  aws_account_id text primary key check (aws_account_id ~ '^[0-9]{12}$'),
  user_id text not null,
  linked_at timestamptz not null default now()
);

create table if not exists oya_browser.aws_marketplace_registrations (
  license_arn text primary key,
  aws_account_id text not null references oya_browser.aws_marketplace_accounts(aws_account_id),
  product_code text not null,
  registered_at timestamptz not null default now()
);

alter table oya_browser.aws_marketplace_accounts enable row level security;
alter table oya_browser.aws_marketplace_registrations enable row level security;
grant all on oya_browser.aws_marketplace_accounts to service_role;
grant all on oya_browser.aws_marketplace_registrations to service_role;
