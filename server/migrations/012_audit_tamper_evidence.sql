-- The audit trail said "append-only" in a comment and nowhere else: service_role
-- held `grant all`, so any code path, backup restore, support script or leaked
-- service key could edit or delete history and leave nothing behind. For a
-- covered entity that is the difference between a log and evidence — HIPAA
-- §164.312(c)(1) asks for a mechanism to authenticate records, and §164.312(b)
-- for audit controls worth examining.
--
-- After this:
--   * every row carries its place in a writer's chain (chain, seq) and the
--     digest linking it to the row before it (prev_hash, hash), so an edit or a
--     deletion is detectable rather than merely forbidden;
--   * update and delete are refused at the table, by grant and by trigger, so
--     "append-only" is enforced where the data lives rather than in the code
--     that writes it.
--
-- Rows written before this migration keep null chain columns; the verifier
-- skips them rather than reporting a break it cannot prove.
--
-- ORDER MATTERS: apply this before deploying the code that writes chain
-- columns. The old code inserts without them, which stays valid; new code
-- against the old schema would insert columns that do not exist and fall back
-- to the audit file.

do $$
begin
  if to_regclass('oya_browser.audit_log') is null then
    raise notice 'oya_browser.audit_log does not exist on this deployment; nothing to migrate';
    return;
  end if;

  alter table oya_browser.audit_log add column if not exists chain text;
  alter table oya_browser.audit_log add column if not exists seq bigint;
  alter table oya_browser.audit_log add column if not exists prev_hash text;
  alter table oya_browser.audit_log add column if not exists hash text;
end $$;

-- One row per position per writer: a replayed insert cannot silently fork a chain.
create unique index if not exists idx_audit_chain_seq
  on oya_browser.audit_log(chain, seq)
  where chain is not null;

-- Verification reads a chain in order; this is the index it walks.
create index if not exists idx_audit_chain on oya_browser.audit_log(chain, seq)
  where chain is not null;

-- ── Append-only, enforced ──
--
-- The grant is the first line: service_role may insert and read, nothing else.
-- The trigger is the second, because a future `grant all` (a migration, a
-- support session, a managed-platform default) would silently undo the first.
--
-- `trigger` goes too, and that is not paperwork: 004's `grant all` left
-- service_role able to drop the very trigger below, which would have made the
-- second line depend on the first one it is meant to survive. `references`
-- goes with it — a foreign key pointed at this table would let a cascade reach
-- rows that update and delete cannot.
--
-- The table owner can still drop a trigger. That is inherent to Postgres, so
-- the owner must not be the role the application authenticates as; the evidence
-- report records which role owns the table.

revoke update, delete, truncate, trigger, references on oya_browser.audit_log from service_role;
grant insert, select on oya_browser.audit_log to service_role;

create or replace function oya_browser.audit_log_append_only()
returns trigger
language plpgsql
as $$
begin
  raise exception 'audit_log is append-only: % refused on row %', tg_op, coalesce(old.id, new.id)
    using errcode = 'restrict_violation',
          hint = 'Correct the record by appending a compensating event.';
end $$;

comment on function oya_browser.audit_log_append_only() is
  'Refuses update and delete on the audit trail (HIPAA 164.312(b),(c)(1)).';

drop trigger if exists audit_log_no_update on oya_browser.audit_log;
create trigger audit_log_no_update
  before update on oya_browser.audit_log
  for each row execute function oya_browser.audit_log_append_only();

drop trigger if exists audit_log_no_delete on oya_browser.audit_log;
create trigger audit_log_no_delete
  before delete on oya_browser.audit_log
  for each row execute function oya_browser.audit_log_append_only();

comment on column oya_browser.audit_log.chain is 'Writer instance chain id; each instance links its own rows.';
comment on column oya_browser.audit_log.seq is 'Position in that chain, from 1.';
comment on column oya_browser.audit_log.prev_hash is 'Hash of the previous row in the chain.';
comment on column oya_browser.audit_log.hash is 'sha256 over prev_hash and this row''s signed fields.';
