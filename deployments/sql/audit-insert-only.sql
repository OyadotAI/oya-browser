-- Give the Oya server a database role that can add to the audit trail but
-- never change or remove it. Run once, by an operator, after the migrations:
--
--   psql "$ADMIN_DATABASE_URL" -v app_password='<new password>' -f deployments/sql/audit-insert-only.sql
--
-- ADMIN_DATABASE_URL is the connection string the server uses today: the role
-- that ran the migrations and owns the tables (on Supabase, `postgres`). The
-- script must run as that role.
--
-- Optional variables:
--   -v app_role=oya_app      name of the new login role (default oya_app)
--
-- What it does:
--   1. creates the login role the server will use from now on (app_role);
--   2. lets it read and write every table in oya_browser, and call every
--      function there, as the server needs;
--   3. on oya_browser.audit_log, leaves it INSERT and SELECT only: no UPDATE,
--      DELETE, TRUNCATE, TRIGGER or REFERENCES;
--   4. keeps audit_log owned by the role running this script, not app_role:
--      a table's owner can drop the append-only triggers (migration 012) and
--      grant itself anything, so the owner must never be the role the server
--      signs in as;
--   5. sets default privileges, so tables and functions later migrations
--      create are usable by app_role without running this again.
--
-- Retention: the server never deletes audit rows (no prune touches audit_log,
-- and erasure leaves it alone on purpose), so app_role needs no delete path.
-- If a retention job for audit_log is ever added, give it a SECURITY DEFINER
-- function owned by the table owner that deletes only rows older than the
-- policy allows, and grant app_role EXECUTE on that function alone.
--
-- After it runs, change the deployment's environment:
--
--   DATABASE_URL=postgres://<app_role>:<app_password>@<host>:<port>/<db>
--       the server's own connection: the new, insert-only role
--   OYA_MIGRATE_DATABASE_URL=<the old DATABASE_URL>
--       migrations run at container start (migrations/run.mjs) and need the
--       owning role; without this they would run as app_role and fail
--
-- On Supabase use the session pooler or direct connection host with the new
-- role's name; the pooler's `postgres.<project-ref>` user form becomes
-- `<app_role>.<project-ref>`.
--
-- Limits, stated plainly: a superuser, and the owning role, can still edit or
-- delete audit rows; that is inherent to Postgres. What this removes is the
-- server's own credential being able to, so a compromised server or a leaked
-- DATABASE_URL cannot rewrite history. The hash chain and its external anchors
-- (OYA_AUDIT_ANCHOR_BUCKET) are what detect a change made by the owner.

\set ON_ERROR_STOP on
\if :{?app_role}
\else
  \set app_role oya_app
\endif
\if :{?app_password}
\else
  \echo 'Set the new role''s password: psql ... -v app_password=...'
  \quit
\endif

select 'do $x$ begin raise exception ''run this as the role that owns the tables, not as app_role''; end $x$'
  where current_user = :'app_role' \gexec

begin;

-- 1. The server's new role. BYPASSRLS because the server enforces tenancy in
-- its own code and the tables' row-level security policies are for the
-- Supabase API roles, as for the role it used before.
select format('create role %I login bypassrls password %L', :'app_role', :'app_password')
  where not exists (select 1 from pg_roles where rolname = :'app_role') \gexec
select format('alter role %I login bypassrls password %L', :'app_role', :'app_password') \gexec

-- 4. The owner stays the role running this script, which is not app_role.
do $$ begin
  if (select tableowner from pg_tables where schemaname = 'oya_browser' and tablename = 'audit_log') <> current_user then
    raise exception 'run this as the role that owns oya_browser.audit_log';
  end if;
end $$;

-- 2. Everything else the server needs.
grant usage on schema oya_browser to :"app_role";
grant select, insert, update, delete on all tables in schema oya_browser to :"app_role";
grant usage, select, update on all sequences in schema oya_browser to :"app_role";
grant execute on all functions in schema oya_browser to :"app_role";

-- 3. The audit trail: add and read, nothing else.
revoke all on oya_browser.audit_log from :"app_role";
grant select, insert on oya_browser.audit_log to :"app_role";
revoke update, delete, truncate, trigger, references on oya_browser.audit_log from public;

-- 5. What later migrations create, run as this role, is usable too. audit_log
-- already exists, so its restriction above is not undone by these.
alter default privileges in schema oya_browser grant select, insert, update, delete on tables to :"app_role";
alter default privileges in schema oya_browser grant usage, select, update on sequences to :"app_role";
alter default privileges in schema oya_browser grant execute on functions to :"app_role";

commit;

-- What app_role may do on audit_log now: expect only INSERT and SELECT.
select privilege_type from information_schema.role_table_grants
  where table_schema = 'oya_browser' and table_name = 'audit_log' and grantee = :'app_role'
  order by 1;
