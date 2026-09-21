-- A captured real-device fingerprint for a persona that mirrors a real
-- browser, or null for the usual generated device. Set once when the persona
-- is created and immutable like the seed: it is the device the persona's
-- cookie jar is paired with.
alter table oya_browser.personas add column if not exists device jsonb;
