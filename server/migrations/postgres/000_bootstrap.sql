-- Plain-Postgres bootstrap. Not needed on Supabase, which already provides both.
--
-- The migrations grant to service_role and live in the oya_browser schema; on
-- Supabase the role and the schema arrive via 001, which cannot run here because
-- it references auth.users. So create just those two things.
--
-- 001, 003, 009 and 010 are accounts, profiles and RLS policies keyed on
-- auth.uid(), and are skipped on plain Postgres (see run.mjs). Everything else,
-- personas, settings, usage, audit and the control plane, is ordinary SQL and
-- runs here too. The server connects as the owning role and bypasses RLS
-- anyway; tenant isolation is enforced in application code.

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

-- api_keys, which 001 creates on Supabase, comes from postgres/001_api_keys.sql.
