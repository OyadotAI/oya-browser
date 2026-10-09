/** Private session ownership outlives context cleanup, preventing stale tabs becoming public. */
import type { Session } from 'electron';
/** Native session identities, never partition strings supplied by a page. */
const records = new WeakMap<Session, { /** Opaque owner. */ owner: object; /** Context identity. */ id: string }>();
/** Mark a session before creating any renderer or permission handler. */
export function ownSession(session: Session, owner: object, id: string): void {
  if (records.has(session)) throw Error('Native context session is already owned');
  records.set(session, { owner, id });
}
/** Tombstoned private sessions must never become default-profile targets. */
export function privateSession(session: Session): boolean {
  return records.has(session);
}
/** Default-profile pages remain available; private pages require the owning connection. */
export function sessionVisible(session: Session, owner: object): boolean {
  const record = records.get(session);
  return !record || record.owner === owner;
}
/** Only the owning connection may disclose a context identity. */
export function sessionContext(session: Session, owner: object): string | undefined {
  const record = records.get(session);
  return record?.owner === owner ? record.id : undefined;
}
