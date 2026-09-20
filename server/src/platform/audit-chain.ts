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
 */

import { createHash, randomBytes } from 'crypto';
import { AUDIT_CHAIN_ID_BYTES, AUDIT_GENESIS_HASH, AUDIT_HASH_ALGORITHM } from './constants.ts';

/** The fields that are hashed, in this order. Anything outside this list is not protected. */
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

/** This process's chain, and where it has got to. */
const state = { chain: randomBytes(AUDIT_CHAIN_ID_BYTES).toString('hex'), seq: 0, prev: AUDIT_GENESIS_HASH };

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
  return SIGNED_FIELDS.map((name) => {
    const value = field(row, name);
    return `${name}:${value.length}:${value}`;
  }).join('\n');
}

/** The digest that links a row to the one before it. */
export function linkHash(prevHash: string, row): string {
  return createHash(AUDIT_HASH_ALGORITHM)
    .update(`${prevHash}\n${canonical(row)}`)
    .digest('hex');
}

/** Stamps a row with its place in this process's chain and advances the chain. */
export function link(row) {
  const placed = { ...row, chain: state.chain, seq: (state.seq += 1), prev_hash: state.prev };
  placed.hash = linkHash(state.prev, placed);
  state.prev = placed.hash;
  return placed;
}

/** Where this process's chain stands, for the evidence report's anchor. */
export function head() {
  return { chain: state.chain, seq: state.seq, hash: state.prev };
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
};

/** Why a row does not follow the one before it, or null when it does. */
function breakReason(row, expectedSeq: number, expectedPrev: string): string | null {
  if (row.seq !== expectedSeq) return `expected seq ${expectedSeq}, found ${row.seq} — a row is missing or reordered`;
  if (row.prev_hash !== expectedPrev) return 'prev_hash does not match the previous row — a row was changed or removed';
  if (row.hash !== linkHash(row.prev_hash, row)) return 'hash does not match the row contents — the row was edited';
  return null;
}

/**
 * Verifies one chain's rows in ascending sequence. Rows of several chains are
 * verified by calling this once per chain: `verifyAll` does that grouping.
 */
export function verifyChain(rows): ChainVerdict {
  const ordered = [...rows].sort((a, b) => a.seq - b.seq);
  let prev = AUDIT_GENESIS_HASH;
  for (const [index, row] of ordered.entries()) {
    const reason = breakReason(row, index + 1, prev);
    if (reason) return { ok: false, checked: index, broken: { chain: row.chain, seq: row.seq, reason } };
    prev = row.hash;
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

/** Verifies every chain present in the rows; the first break found wins. */
export function verifyAll(rows): ChainsVerdict {
  const chains = byChain(rows.filter((r) => r.chain));
  let checked = 0;
  for (const [, chainRows] of chains) {
    const verdict = verifyChain(chainRows);
    checked += verdict.checked;
    if (!verdict.ok) return { ...verdict, checked, chains: chains.size };
  }
  return { ok: true, checked, chains: chains.size };
}
