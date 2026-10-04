-- Behind the auth middleware every request carries its project's key, so the
-- audit actor (a key fingerprint) named the project and never the person: a
-- member's credential, an operator share and the owner all looked the same.
-- These columns keep who actually authenticated: the credential id, the member
-- it was issued to, and its role. All nullable; rows written before this, and
-- requests with no project credential, leave them null.
--
-- Chains written by the new code are keyed (HMAC) and sign these columns too;
-- their chain ids start with 'h-'. Older chains stay plain SHA-256 and verify
-- as before.
--
-- ORDER MATTERS: apply this before deploying the code that writes these
-- columns, or its inserts fail: events queue in memory, then spill to the
-- audit overflow file (data/audit-overflow.jsonl).

do $$
begin
  if to_regclass('oya_browser.audit_log') is null then
    raise notice 'oya_browser.audit_log does not exist on this deployment; nothing to migrate';
    return;
  end if;

  alter table oya_browser.audit_log add column if not exists credential_id text;
  alter table oya_browser.audit_log add column if not exists member_user text;
  alter table oya_browser.audit_log add column if not exists actor_role text;
end $$;

comment on column oya_browser.audit_log.credential_id is 'Project credential that authenticated the request, when there was one.';
comment on column oya_browser.audit_log.member_user is 'Project member the credential was issued to.';
comment on column oya_browser.audit_log.actor_role is 'Role of that credential: administrator, operator, viewer or browser.';
comment on column oya_browser.audit_log.hash is 'HMAC-SHA256 (chain ids starting h-) or sha256 (older chains) over prev_hash and the row''s signed fields.';
