-- Durable control plane: versioned rows, an append-only event log, and per-session command gates.
-- Implements the backend contract of server/src/control/store.js. Only the service role may call these functions.
create table if not exists oya_browser.control_rows (
  kind text not null, id text not null, project text, state text, expires_at bigint,
  body jsonb not null, version bigint not null,
  primary key (kind, id)
);
create index if not exists control_rows_project on oya_browser.control_rows (kind, project);
create index if not exists control_rows_state on oya_browser.control_rows (kind, state);
create index if not exists control_rows_expiry on oya_browser.control_rows (expires_at) where expires_at is not null;
create table if not exists oya_browser.control_events (
  seq bigserial primary key, project text not null, type text not null, session_id text,
  at bigint not null, detail jsonb not null default '{}'::jsonb
);
create index if not exists control_events_project on oya_browser.control_events (project, seq);
-- Command coordination is per session: command traffic never rewrites session rows.
create table if not exists oya_browser.control_gates (
  id text primary key, state text not null, mode text not null, holder text,
  expires_at bigint, fence bigint not null, in_flight integer not null default 0, owner text
);
alter table oya_browser.control_gates add column if not exists owner text;
alter table oya_browser.control_rows enable row level security;
alter table oya_browser.control_events enable row level security;
alter table oya_browser.control_gates enable row level security;

-- Retire the pre-release single-document functions if an earlier draft of this migration was installed.
drop function if exists oya_browser.control_read();
drop function if exists oya_browser.control_commit(bigint, jsonb);
drop function if exists oya_browser.control_begin(text, text);

create or replace function oya_browser.control_load(queries jsonb) returns jsonb
language plpgsql stable security definer set search_path = '' as $$
declare q jsonb; result jsonb := '[]'::jsonb; matched jsonb;
begin
  for q in select value from jsonb_array_elements(queries) loop
    select coalesce(jsonb_agg(jsonb_build_object('id', r.id, 'version', r.version,
        'body', case when r.kind = 'session' then r.body || jsonb_build_object('inFlight', coalesce(g.in_flight, 0)) else r.body end)), '[]'::jsonb)
      into matched
      from oya_browser.control_rows r
      left join oya_browser.control_gates g on r.kind = 'session' and g.id = r.id
      where r.kind = q->>'kind'
        and (not q ? 'id' or r.id = q->>'id')
        and (not q ? 'project' or r.project = q->>'project')
        and (not q ? 'states' or r.state in (select jsonb_array_elements_text(q->'states')));
    result := result || jsonb_build_array(matched);
  end loop;
  return result;
end $$;

-- Applies every write or none. A version mismatch returns ok=false; a racing insert raises control_conflict.
create or replace function oya_browser.control_commit(writes jsonb, events jsonb) returns jsonb
language plpgsql security definer set search_path = '' as $$
declare w jsonb; e jsonb; current_version bigint; gate oya_browser.control_gates%rowtype; seq_value bigint; seqs jsonb := '[]'::jsonb;
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
  for e in select value from jsonb_array_elements(events) loop
    insert into oya_browser.control_events (project, type, session_id, at, detail)
      values (e->>'project', e->>'type', e->>'sessionId', (e->>'at')::bigint, coalesce(e->'detail', '{}'::jsonb))
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

create or replace function oya_browser.control_read_events(target_project text, after_seq bigint, lim integer, latest boolean, seqs jsonb) returns jsonb
language sql stable security definer set search_path = '' as $$
  select coalesce(jsonb_agg(jsonb_build_object('id', x.seq, 'project', x.project, 'type', x.type, 'sessionId', x.session_id, 'at', x.at, 'detail', x.detail) order by x.seq), '[]'::jsonb)
  from (
    select * from oya_browser.control_events ev
    where case when seqs is not null then ev.seq in (select (jsonb_array_elements_text(seqs))::bigint)
               else ev.project = target_project and ev.seq > coalesce(after_seq, 0) end
    order by case when latest then -ev.seq else ev.seq end
    limit case when seqs is not null then null else lim end
  ) x
$$;

create or replace function oya_browser.control_prune(now_ms bigint, cutoffs jsonb) returns void
language plpgsql security definer set search_path = '' as $$
declare c record;
begin
  delete from oya_browser.control_gates g using oya_browser.control_rows r where r.kind = 'session' and r.id = g.id and r.expires_at < now_ms;
  delete from oya_browser.control_rows where expires_at < now_ms;
  for c in select key, value from jsonb_each_text(cutoffs) loop
    delete from oya_browser.control_events where project = c.key and at < c.value::bigint;
  end loop;
end $$;

create or replace function oya_browser.control_begin(session_id text, actor text, caller_instance text) returns bigint
language plpgsql security definer set search_path = '' as $$
declare gate oya_browser.control_gates%rowtype;
begin
  select * into gate from oya_browser.control_gates where id = session_id for update;
  if not found then return null; end if;
  if (gate.owner is not null and gate.owner is distinct from caller_instance) or gate.state <> 'ready' or not coalesce(((gate.mode = 'agent' and actor is null) or (gate.mode = 'human' and gate.holder = actor and gate.expires_at > extract(epoch from clock_timestamp()) * 1000)), false) then raise exception 'control_paused'; end if;
  update oya_browser.control_gates set in_flight = in_flight + 1 where id = session_id;
  return gate.fence;
end $$;

create or replace function oya_browser.control_finish(session_id text, generation bigint) returns void
language sql security definer set search_path = '' as $$
  update oya_browser.control_gates set in_flight = greatest(0, in_flight - 1) where id = session_id and fence = generation;
$$;

grant usage on schema oya_browser to service_role;
revoke all on function oya_browser.control_load(jsonb) from public, anon, authenticated;
revoke all on function oya_browser.control_commit(jsonb, jsonb) from public, anon, authenticated;
revoke all on function oya_browser.control_read_events(text, bigint, integer, boolean, jsonb) from public, anon, authenticated;
revoke all on function oya_browser.control_prune(bigint, jsonb) from public, anon, authenticated;
revoke all on function oya_browser.control_begin(text, text, text) from public, anon, authenticated;
revoke all on function oya_browser.control_finish(text, bigint) from public, anon, authenticated;
grant execute on function oya_browser.control_load(jsonb) to service_role;
grant execute on function oya_browser.control_commit(jsonb, jsonb) to service_role;
grant execute on function oya_browser.control_read_events(text, bigint, integer, boolean, jsonb) to service_role;
grant execute on function oya_browser.control_prune(bigint, jsonb) to service_role;
grant execute on function oya_browser.control_begin(text, text, text) to service_role;
grant execute on function oya_browser.control_finish(text, bigint) to service_role;

-- Operator-created service keys need no interactive account.
alter table if exists oya_browser.api_keys alter column user_id drop not null;

-- Make the new functions callable through PostgREST immediately.
notify pgrst, 'reload schema';
