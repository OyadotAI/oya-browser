-- Close a hole in 001 where the database trusted the application to hold a line
-- the application states in prose.
--
-- 1. profiles: the UPDATE policy had a USING clause and no WITH CHECK, and the
--    grant covered every column. `auth.uid() = id` is true both before and
--    after `set role = 'admin'`, so any signed-in user could promote itself
--    with one PostgREST call. auth.js's updateProfile() only ever writes
--    display_name and says why ("a profile form that could raise its own role
--    would be a privilege escalation with a text input in front of it") — this
--    makes the database say the same thing.
-- 2. api_keys: handled in 010, which stops storing the key at all and takes the
--    table's grants away from `authenticated` entirely.

-- ── profiles: display_name is the only thing a user may change about itself ──

drop policy if exists "Users can update own profile" on oya_browser.profiles;
create policy "Users can update own profile"
  on oya_browser.profiles for update
  using (auth.uid() = id)
  with check (auth.uid() = id);

-- INSERT and DELETE carry no policy, so the grants were already inert; drop
-- them so the grant list stops implying an access path that does not exist.
revoke insert, update, delete on oya_browser.profiles from authenticated;
grant update (display_name) on oya_browser.profiles to authenticated;
