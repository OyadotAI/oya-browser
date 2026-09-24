/**
 * SQL both SQL drivers share. Only the placeholder differs ($1 in Postgres, ?
 * in SQLite), so each statement is built with a `bind` that records a value
 * and answers the placeholder for it.
 */
import type { Where, SelectOptions } from './connection.ts';

/** Records a value as a parameter and answers its placeholder. */
export type Bind = (value: unknown) => string;

/** One filter entry as SQL. */
function test(column: string, value: any, bind: Bind) {
  if (value === null) return `${column} is null`;
  if (value && typeof value === 'object' && 'gte' in value) return `${column} >= ${bind(value.gte)}`;
  if (value && typeof value === 'object' && 'notIn' in value)
    return value.notIn.length ? `${column} not in (${value.notIn.map(bind).join(', ')})` : '';
  return `${column} = ${bind(value)}`;
}

/** ` where …` for a filter, or '' when it has no entries. */
export function whereSql(where: Where = {}, bind: Bind) {
  const tests = Object.entries(where)
    .map(([column, value]) => test(column, value, bind))
    .filter(Boolean);
  return tests.length ? ` where ${tests.join(' and ')}` : '';
}

/** ` order by … limit …` for select options. */
export function tailSql({ order, limit }: SelectOptions = {}, bind: Bind) {
  const sort = order ? ` order by ${order[0]} ${order[1] === 'desc' ? 'desc' : 'asc'}` : '';
  return sort + (limit ? ` limit ${bind(limit)}` : '');
}

/** ` on conflict (key) do …`: replace every non-key column, or leave the stored row. */
export function conflictSql(key: string[], columns: string[], update: boolean) {
  const set = columns.filter((c) => !key.includes(c)).map((c) => `${c} = excluded.${c}`);
  return ` on conflict (${key.join(', ')}) ${update && set.length ? `do update set ${set.join(', ')}` : 'do nothing'}`;
}

/** `a = $1, b = $2` for an update. */
export const setSql = (set: Record<string, unknown>, bind: Bind) =>
  Object.entries(set)
    .map(([column, value]) => `${column} = ${bind(value)}`)
    .join(', ');
