/**
 * The durable control plane on Postgres. The `{ rpc }` client RemoteBackend
 * speaks to now lives with the rest of the Postgres driver in
 * platform/storage; this re-exports it under the names callers already use.
 *
 * Apply the schema first with `node server/migrations/run.mjs`.
 */
export { pgRemote, closePgPool } from '../../platform/storage/index.ts';
