-- Make the audit anchor bucket write-once. Run once against the Supabase
-- project's database (the one SUPABASE_URL names), as `postgres`:
--
--   psql "$SUPABASE_DB_URL" -v bucket=oya-audit-anchors -f deployments/sql/audit-anchor-bucket.sql
--
-- then set OYA_AUDIT_ANCHOR_BUCKET=oya-audit-anchors on the server.
--
-- Why a trigger and not a storage policy: the server talks to Supabase Storage
-- with the service role key, and the service role bypasses storage policies.
-- A trigger on storage.objects fires whoever asks, so an update (an
-- overwrite), a rename or a delete of an object in this bucket is refused for
-- the server's key as for everyone else. The server never needs either: it adds
-- anchors with upsert disabled.
--
-- What it does not stop: a database owner dropping the trigger, or someone
-- with access to the storage backend itself. For anchors that outlive a
-- compromised Supabase project, replicate the bucket to S3 or GCS with a
-- retention lock (Object Lock in compliance mode, or a locked retention policy).

\set ON_ERROR_STOP on
\if :{?bucket}
\else
  \set bucket oya-audit-anchors
\endif

begin;

-- The bucket, private.
insert into storage.buckets (id, name, public) values (:'bucket', :'bucket', false)
  on conflict (id) do update set public = false;

-- Refuses deleting, renaming, moving or overwriting (a new version of) an
-- object in the bucket named by the trigger's argument. Metadata-only updates,
-- which Storage itself makes, pass: verification reads object names only.
create or replace function public.oya_audit_anchor_write_once() returns trigger
language plpgsql set search_path = '' as $$
begin
  if old.bucket_id = tg_argv[0] and (tg_op = 'DELETE'
      or (new.bucket_id, new.name, new.version) is distinct from (old.bucket_id, old.name, old.version)) then
    raise exception 'audit anchors are write-once: % refused on %', tg_op, old.name
      using errcode = 'restrict_violation';
  end if;
  return case when tg_op = 'DELETE' then old else new end;
end $$;

drop trigger if exists oya_audit_anchor_write_once on storage.objects;
create trigger oya_audit_anchor_write_once
  before update or delete on storage.objects
  for each row execute function public.oya_audit_anchor_write_once(:'bucket');

commit;
