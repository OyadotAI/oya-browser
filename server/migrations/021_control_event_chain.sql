-- Tamper evidence for control_events: each event carries the hash of the
-- event before it in its project, so an edited event, or one removed from the
-- middle of a project's history, breaks a link that control_verify_events
-- (and GET /operator/audit/verify?project=) reports.
--
-- Why plain sha256, not an HMAC: the hash is computed here, inside
-- control_commit, because several replicas commit events and only the
-- database orders them. A key the database could read would be readable by
-- the very role that can rewrite the table, so it would add nothing against
-- that writer, and pgcrypto lives in a different schema on Supabase
-- (extensions) than on plain Postgres. sha256() is built in (Postgres 11+).
-- What this catches: accidental edits, partial edits, and rows deleted from
-- the middle. What it does not: someone who rewrites a project's events and
-- recomputes every link after them, or who deletes the newest events. Those
-- need the heads anchored outside the database, as the audit trail's are.
--
-- The text hashed must stay identical to server/src/modules/control/store/
-- event-chain.ts (the SQLite backend): prev_hash, project, type, session_id
-- ('' when null), at and detail as text, joined by newlines.
--
-- Events written before this migration keep null links and are not checked.
-- A commit already running when this is applied uses the old function and
-- writes an unlinked event; the next event links past it to the last linked one.

alter table oya_browser.control_events add column if not exists prev_hash text;
alter table oya_browser.control_events add column if not exists hash text;

create or replace function oya_browser.control_event_hash(prev text, project text, type text, session_id text, at bigint, detail jsonb) returns text
language sql immutable set search_path = '' as $$
  select encode(sha256(convert_to(prev || E'\n' || project || E'\n' || type || E'\n' || coalesce(session_id, '') || E'\n' || at::text || E'\n' || detail::text, 'UTF8')), 'hex')
$$;

-- As in 008, with each event linked to its project's previous one. Replicas
-- commit concurrently, so the link is read under a per-project advisory lock
-- held to the end of the transaction: the next writer for that project waits,
-- then (read committed, a fresh snapshot per statement) sees this event.
-- Locks are taken in project order so two commits cannot deadlock on them.
create or replace function oya_browser.control_commit(writes jsonb, events jsonb) returns jsonb
language plpgsql security definer set search_path = '' as $$
declare w jsonb; e jsonb; p text; current_version bigint; gate oya_browser.control_gates%rowtype; seq_value bigint; seqs jsonb := '[]'::jsonb;
  prev text; event_detail jsonb;
begin
  for w in select value from jsonb_array_elements(writes) loop
    select version into current_version from oya_browser.control_rows where kind = w->>'kind' and id = w->>'id' for update;
    if coalesce(current_version, 0) <> (w->>'version')::bigint then return jsonb_build_object('ok', false); end if;
  end loop;
  for w in select value from jsonb_array_elements(writes) loop
    if jsonb_typeof(w->'body') is distinct from 'object' then
      delete from oya_browser.control_rows where kind = w->>'kind' and id = w->>'id';
      if w->>'kind' = 'session' then delete from oya_browser.control_gates where id = w->>'id'; end if;
      continue;
    end if;
    if (w->>'version')::bigint = 0 then
      insert into oya_browser.control_rows values (w->>'kind', w->>'id', w->>'project', w->>'state', (w->>'expiresAt')::bigint, w->'body', 1) on conflict do nothing;
      if not found then raise exception 'control_conflict'; end if;
    else
      update oya_browser.control_rows set project = w->>'project', state = w->>'state', expires_at = (w->>'expiresAt')::bigint, body = w->'body', version = version + 1
        where kind = w->>'kind' and id = w->>'id';
    end if;
    if w->>'kind' = 'session' then
      select * into gate from oya_browser.control_gates where id = w->>'id' for update;
      if gate.in_flight > 0 and w#>>'{body,control,mode}' = 'human' and gate.mode <> 'human' then raise exception 'commands_pending'; end if;
      insert into oya_browser.control_gates (id, state, mode, holder, expires_at, fence, in_flight, owner)
        values (w->>'id', w#>>'{body,state}', coalesce(w#>>'{body,control,mode}', 'agent'), w#>>'{body,control,holder}', (w#>>'{body,control,expiresAt}')::bigint, coalesce((w#>>'{body,fence}')::bigint, 0), 0, w#>>'{body,instance}')
        on conflict (id) do update set owner = excluded.owner, state = excluded.state, mode = excluded.mode, holder = excluded.holder, expires_at = excluded.expires_at,
          in_flight = case when control_gates.fence <> excluded.fence then 0 else control_gates.in_flight end, fence = excluded.fence;
    end if;
  end loop;
  for p in select distinct x.value->>'project' from jsonb_array_elements(events) x order by 1 loop
    perform pg_advisory_xact_lock(hashtextextended('oya_browser.control_events:' || p, 0));
  end loop;
  for e in select value from jsonb_array_elements(events) loop
    prev := null;
    select ev.hash into prev from oya_browser.control_events ev where ev.project = e->>'project' and ev.hash is not null order by ev.seq desc limit 1;
    prev := coalesce(prev, repeat('0', 64));
    event_detail := coalesce(e->'detail', '{}'::jsonb);
    insert into oya_browser.control_events (project, type, session_id, at, detail, prev_hash, hash)
      values (e->>'project', e->>'type', e->>'sessionId', (e->>'at')::bigint, event_detail, prev,
        oya_browser.control_event_hash(prev, e->>'project', e->>'type', e->>'sessionId', (e->>'at')::bigint, event_detail))
      returning seq into seq_value;
    seqs := seqs || to_jsonb(seq_value);
    -- Deliveries are created with their event, so a crash cannot lose a notification.
    insert into oya_browser.control_rows (kind, id, project, state, expires_at, body, version)
      select 'delivery', h.id || ':' || seq_value, h.project, 'pending', null,
        jsonb_build_object('id', h.id || ':' || seq_value, 'hook', h.id, 'project', h.project, 'eventSeq', seq_value, 'at', (e->>'at')::bigint, 'attempts', 0, 'nextAt', (e->>'at')::bigint, 'state', 'pending'), 1
      from oya_browser.control_rows h
      where h.kind = 'webhook' and h.project = e->>'project' and (h.body->>'enabled')::boolean
        and (jsonb_array_length(h.body->'types') = 0 or h.body->'types' ? (e->>'type'));
  end loop;
  return jsonb_build_object('ok', true, 'events', seqs);
end $$;

-- Retention cuts a project's history from the start only: everything before
-- its first event at or after the cutoff. Replica clocks can stamp a later
-- event earlier; deleting by time alone could leave a hole in the middle,
-- which the chain would report as a removed event.
create or replace function oya_browser.control_prune(now_ms bigint, cutoffs jsonb) returns void
language plpgsql security definer set search_path = '' as $$
declare c record;
begin
  delete from oya_browser.control_gates g using oya_browser.control_rows r where r.kind = 'session' and r.id = g.id and r.expires_at < now_ms;
  delete from oya_browser.control_rows where expires_at < now_ms;
  for c in select key, value from jsonb_each_text(cutoffs) loop
    delete from oya_browser.control_events where project = c.key
      and seq < coalesce((select min(k.seq) from oya_browser.control_events k where k.project = c.key and k.at >= c.value::bigint), 9223372036854775807);
  end loop;
end $$;

-- Checks a project's newest `lim` linked events, oldest first, trusting the
-- first one's link backwards: retention prunes the start. Answers the same
-- shape, and the same reasons, as verifyEventChain in event-chain.ts.
create or replace function oya_browser.control_verify_events(target_project text, lim integer) returns jsonb
language plpgsql stable security definer set search_path = '' as $$
declare r record; expected text := null; checked integer := 0;
begin
  for r in select * from (
      select ev.* from oya_browser.control_events ev
      where ev.project = target_project and ev.hash is not null
      order by ev.seq desc limit lim) x
    order by x.seq loop
    if expected is not null and r.prev_hash is distinct from expected then
      return jsonb_build_object('ok', false, 'checked', checked, 'broken',
        jsonb_build_object('seq', r.seq, 'reason', 'prev_hash does not match the previous event, an event was removed'));
    end if;
    if r.hash is distinct from oya_browser.control_event_hash(r.prev_hash, r.project, r.type, r.session_id, r.at, r.detail) then
      return jsonb_build_object('ok', false, 'checked', checked, 'broken',
        jsonb_build_object('seq', r.seq, 'reason', 'hash does not match the event, it was edited'));
    end if;
    expected := r.hash;
    checked := checked + 1;
  end loop;
  return jsonb_build_object('ok', true, 'checked', checked);
end $$;

revoke all on function oya_browser.control_event_hash(text, text, text, text, bigint, jsonb) from public, anon, authenticated;
revoke all on function oya_browser.control_verify_events(text, integer) from public, anon, authenticated;
grant execute on function oya_browser.control_event_hash(text, text, text, text, bigint, jsonb) to service_role;
grant execute on function oya_browser.control_verify_events(text, integer) to service_role;

notify pgrst, 'reload schema';
