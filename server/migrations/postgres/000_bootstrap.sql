-- Plain-Postgres bootstrap. Not needed on Supabase, which already provides both.
--
-- 008_durable_control.sql grants execute to service_role and lives in the
-- oya_browser schema; on Supabase the role and the schema arrive via 001, which
-- cannot run here because it references auth.users. So create just the two
-- things 008 actually depends on.
--
-- 001-007 are accounts, profiles and RLS policies keyed on auth.uid(). They are
-- deliberately skipped on plain Postgres: the server connects as the owning role
-- and bypasses RLS anyway, tenant isolation is enforced in application code
-- (projectId = 'prj_' + sha256(apiKey), see server/src/control/service.js), and
-- authentication on this path is API_KEYS rather than Supabase Auth.

create schema if not exists oya_browser;

-- service_role is the role the server connects as. anon and authenticated exist
-- only so 008's defence-in-depth `revoke ... from public, anon, authenticated`
-- still parses: on Supabase those are the roles PostgREST exposes functions to.
-- There is no PostgREST here, so they are created unable to log in and left empty.
do $$
declare r text;
begin
  foreach r in array array['service_role', 'anon', 'authenticated'] loop
    if not exists (select 1 from pg_roles where rolname = r) then
      execute format('create role %I nologin noinherit', r);
    end if;
  end loop;
end
$$;

grant usage on schema oya_browser to service_role;

-- api_keys is created by 001 on Supabase. On this path the control store holds
-- service credentials itself, but 008 ends with an ALTER on this table; the
-- "if exists" there makes it a no-op, so nothing more is needed.
