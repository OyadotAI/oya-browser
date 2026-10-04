/**
 * Tamper evidence for the audit trail.
 *
 * HIPAA §164.312(c)(1) asks for a mechanism to authenticate records: not that
 * the log exists, but that what it says now is what it said then. An
 * append-only table is a convention; a hash chain is a proof. Each row carries
 * the digest of the row before it, so editing or removing any row breaks every
 * link after it and `verify` can name the first one that no longer adds up.
 *
 * Each server instance keeps its own chain, because a single shared chain would
 * need the instances to agree on an order before they could write. A chain id
 * plus a per-chain sequence gives every row a place with no coordination, and
 * verification runs per chain.
 *
 * A plain hash proves nothing to someone who can write the table: they can edit
 * a row and recompute every link after it. So chains are keyed: each link is an
 * HMAC under OYA_AUDIT_HMAC_KEY, or, when that is unset, under a key derived
 * from the server secret. Chains written before that (ids without the HMAC
 * prefix) still verify as plain SHA-256 and are counted as unkeyed.
 */

import { createHash, createHmac, randomBytes } from 'crypto';
import { derivedKey } from './secrets.ts';
import {
  AUDIT_CHAIN_ID_BYTES,
  AUDIT_GENESIS_HASH,
  AUDIT_HASH_ALGORITHM,
  AUDIT_HMAC_ALGORITHM,
  AUDIT_HMAC_CHAIN_PREFIX,
  AUDIT_HMAC_KEY_PURPOSE,
  auditHmacKey,
} from './constants.ts';

/** The fields an unkeyed (legacy) chain hashed, in this order. Anything outside this list is not protected. */
const SIGNED_FIELDS = [
  'chain',
  'seq',
  'ts',
  'action',
  'actor',
  'actor_user',
  'target_type',
  'target_id',
  'outcome',
  'ip',
  'user_agent',
  'meta',
] as const;

/** The fields a keyed chain signs: the legacy ones plus who acted, as the credential that authenticated them. */
const KEYED_FIELDS = [...SIGNED_FIELDS, 'credential_id', 'member_user', 'actor_role'] as const;

/** The key derived from the server secret, once loaded; null until a keyed link needs it. */
let derived: Buffer | null = null;

/** The key derived from the server secret; throws when there is no secret to derive from. */
function derivedChainKey(): Buffer {
  derived ??= derivedKey(AUDIT_HMAC_KEY_PURPOSE);
  return derived;
}

/** The key keyed links are signed with now: the operator's, else the derived one. */
function chainKey(): Buffer {
  const explicit = auditHmacKey();
  return explicit ? Buffer.from(explicit) : derivedChainKey();
}

/**
 * Every key a stored keyed link may be under. Links carry no key id, so an
 * operator setting OYA_AUDIT_HMAC_KEY on a running deployment would otherwise
 * make every chain signed under the derived key read as tampered.
 */
function verifyKeys(): Buffer[] {
  const keys = [chainKey()];
  try {
    if (auditHmacKey()) keys.push(derivedChainKey());
  } catch {
    // No server secret: only the operator's key can have signed anything.
  }
  return keys;
}

/** Whether a chain id names a keyed (HMAC) chain. */
const isKeyed = (chain) => typeof chain === 'string' && chain.startsWith(AUDIT_HMAC_CHAIN_PREFIX);

/** Where one writer's chain has got to. */
type ChainState = {
  /** The chain id; a keyed chain's starts with the HMAC prefix. */
  chain: string;
  /** The last sequence number written. */
  seq: number;
  /** The last hash written, which the next row links to. */
  prev: string;
};

/** This process's chain; started on first use. */
let state: ChainState | null = null;

/** A new keyed chain, or an unkeyed one, said loudly, when no key can be had: an audit write must not fail. */
function startChain() {
  const id = randomBytes(AUDIT_CHAIN_ID_BYTES).toString('hex');
  try {
    chainKey();
    return { chain: AUDIT_HMAC_CHAIN_PREFIX + id, seq: 0, prev: AUDIT_GENESIS_HASH };
  } catch (e) {
    console.error(`[audit] NO CHAIN KEY (${e.message}): this process writes an UNKEYED chain; set OYA_AUDIT_HMAC_KEY`);
    return { chain: id, seq: 0, prev: AUDIT_GENESIS_HASH };
  }
}

/** This process's chain. */
const current = () => (state ??= startChain());

/**
 * A value as the digest sees it. Keys are sorted because Postgres `jsonb` does
 * not keep the order they were written in, and a round trip through it must not
 * change the hash.
 */
function stable(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value ?? null);
  if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
  const keys = Object.keys(value).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stable(value[k])}`).join(',')}}`;
}

/**
 * The timestamp as the digest sees it: epoch milliseconds, because the database
 * hands back `+00:00` where the writer wrote `Z` and both mean one instant.
 */
function instant(ts) {
  const ms = Date.parse(ts);
  return Number.isNaN(ms) ? String(ts) : String(ms);
}

/** One field as the digest sees it. */
function field(row, name) {
  if (name === 'ts') return instant(row.ts);
  if (name === 'meta') return stable(row.meta ?? null);
  return stable(row[name] ?? null);
}

/** The exact bytes hashed for a row: the signed fields, in order, length-prefixed. */
export function canonical(row): string {
  return (isKeyed(row.chain) ? KEYED_FIELDS : SIGNED_FIELDS)
    .map((name) => {
      const value = field(row, name);
      return `${name}:${value.length}:${value}`;
    })
    .join('\n');
}

/** The digest that links a row to the one before it: an HMAC on a keyed chain, plain SHA-256 on a legacy one. */
export function linkHash(prevHash: string, row, key: Buffer = chainKey()): string {
  const digest = isKeyed(row.chain) ? createHmac(AUDIT_HMAC_ALGORITHM, key) : createHash(AUDIT_HASH_ALGORITHM);
  return digest.update(`${prevHash}\n${canonical(row)}`).digest('hex');
}

/** Whether a stored row's hash is its link under any key it may have been signed with. */
const linkMatches = (row) =>
  isKeyed(row.chain)
    ? verifyKeys().some((key) => row.hash === linkHash(row.prev_hash, row, key))
    : row.hash === linkHash(row.prev_hash, row);

/** Stamps a row with its place in this process's chain and advances the chain. */
export function link(row) {
  const chain = current();
  const placed = { ...row, chain: chain.chain, seq: (chain.seq += 1), prev_hash: chain.prev };
  placed.hash = linkHash(chain.prev, placed);
  chain.prev = placed.hash;
  return placed;
}

/** A keyed digest of `text` under the chain key: what an anchor is signed with. */
export const seal = (text: string) => createHmac(AUDIT_HMAC_ALGORITHM, chainKey()).update(text).digest('hex');

/** Whether `sig` seals `text` under any key a chain may have been signed with. */
export const sealMatches = (text: string, sig: string) =>
  verifyKeys().some((key) => createHmac(AUDIT_HMAC_ALGORITHM, key).update(text).digest('hex') === sig);

/** Where this process's chain stands, for the evidence report's anchor. */
export function head() {
  const { chain, seq, prev } = current();
  return { chain, seq, hash: prev };
}

/** The first row that does not add up, and why. */
export type ChainBreak = {
  /** Which writer's chain it belongs to. */
  chain: string;
  /** Its position in that chain. */
  seq: number;
  /** What did not add up, in words an auditor can act on. */
  reason: string;
};

/** What a verification found: intact, or the first row that does not add up. */
export type ChainVerdict = {
  /** True when every row links to the one before it with no gaps. */
  ok: boolean;
  /** How many rows were checked. */
  checked: number;
  /** The first row that failed, when one did. */
  broken?: ChainBreak;
};

/** A verdict over several chains at once. */
export type ChainsVerdict = ChainVerdict & {
  /** How many distinct writer chains were present. */
  chains: number;
  /** Rows on unkeyed (plain SHA-256) chains: consistent, but recomputable by anyone who can write the table. */
  unkeyed: number;
};

/** Why a row does not follow the one before it, or null when it does. */
function breakReason(row, expectedSeq: number, expectedPrev: string): string | null {
  if (row.seq !== expectedSeq) return `expected seq ${expectedSeq}, found ${row.seq}, a row is missing or reordered`;
  if (row.prev_hash !== expectedPrev) return 'prev_hash does not match the previous row, a row was changed or removed';
  if (!linkMatches(row)) return 'hash does not match the row contents, the row was edited';
  return null;
}

/** Where a chain read from its beginning starts. */
const GENESIS = { seq: 1, prev_hash: AUDIT_GENESIS_HASH };

/**
 * Verifies one chain's rows in ascending sequence. Rows of several chains are
 * verified by calling this once per chain: `verifyAll` does that grouping.
 * `anchored` verifies a window that does not start at the genesis, trusting
 * the first row's place and prev_hash: what a bounded read of the newest rows needs.
 */
export function verifyChain(rows, anchored = false): ChainVerdict {
  const ordered = [...rows].sort((a, b) => a.seq - b.seq);
  const first = anchored && ordered.length ? ordered[0] : GENESIS;
  for (const [index, row] of ordered.entries()) {
    const reason = breakReason(row, first.seq + index, index ? ordered[index - 1].hash : first.prev_hash);
    if (reason) return { ok: false, checked: index, broken: { chain: row.chain, seq: row.seq, reason } };
  }
  return { ok: true, checked: ordered.length };
}

/** Groups rows by chain, so each chain is verified from its own genesis. */
function byChain(rows): Map<string, any[]> {
  const chains = new Map();
  for (const row of rows) {
    if (!chains.has(row.chain)) chains.set(row.chain, []);
    chains.get(row.chain).push(row);
  }
  return chains;
}

/** Verifies every chain present in the rows (each as a window when `anchored`); the first break found wins. */
export function verifyAll(rows, anchored = false): ChainsVerdict {
  const chains = byChain(rows.filter((r) => r.chain));
  const tally = { checked: 0, chains: chains.size, unkeyed: rows.filter((r) => r.chain && !isKeyed(r.chain)).length };
  for (const [, chainRows] of chains) {
    const verdict = verifyChain(chainRows, anchored);
    tally.checked += verdict.checked;
    if (!verdict.ok) return { ...verdict, ...tally };
  }
  return { ok: true, ...tally };
}
