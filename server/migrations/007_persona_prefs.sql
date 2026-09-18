-- Device choices a persona made at creation: platform, timezone, locale.
-- Stored with the seed and just as immutable — together they are the
-- fingerprint, and the fingerprint is what must not change under a cookie jar.
alter table oya_browser.personas add column if not exists prefs jsonb;
