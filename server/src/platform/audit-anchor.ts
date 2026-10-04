/**
 * Anchors for the audit chains: evidence kept outside the database.
 *
 * A hash chain shows a changed or missing row in the middle, but not a chain
 * cut short at its end, or one deleted whole: what is left still links up.
 * So every so often each process writes the head of its chain (its newest
 * stored row's position and hash), sealed with the chain key, to a storage
 * bucket the application may add to but not overwrite or delete from. At
 * startup the newest anchor of every chain is compared with the database: an
 * anchored row that is gone, or reads differently, means rows were removed or
 * rewritten after the anchor was taken.
 *
 * Everything verification needs is in the object name, so checking an anchor
 * is a listing, never a download. The body repeats it with the time, for a
 * person reading the bucket.
 */

import { seal, sealMatches } from './audit-chain.ts';
import {
  AUDIT_ANCHOR_LIST_MAX,
  AUDIT_ANCHOR_SEQ_DIGITS,
  AUDIT_ANCHOR_VERIFY_DAYS,
  AUDIT_DAY_MS,
  ISO_DATE_CHARS,
} from './constants.ts';

/** A storage failure as the bucket client reports it. */
type StorageError = {
  /** What the storage service said. */
  message: string;
};

/** How an anchor is uploaded. */
type UploadOptions = {
  /** The body's media type. */
  contentType: string;
  /** Always false: an existing name is an error, never an overwrite. */
  upsert: boolean;
};

/** What a bucket call answers: data, or why it failed. */
type StorageResult<T> = {
  /** The call's result. */
  data?: T | null;
  /** The failure, when there was one. */
  error?: StorageError | null;
};

/** One object in a listing. */
type StoredObject = {
  /** Its name within the listed folder. */
  name: string;
};

/** How many objects a listing returns. */
type ListOptions = {
  /** At most this many. */
  limit: number;
};

/** The bucket calls anchoring needs: Supabase storage's `from(bucket)`, or a fake in tests. */
export type AnchorObjects = {
  /** Adds an object; `upsert: false` makes an existing name an error rather than an overwrite. */
  upload(path: string, body: string, options: UploadOptions): Promise<StorageResult<unknown>>;
  /** Names of the objects under a folder. */
  list(prefix: string, options: ListOptions): Promise<StorageResult<StoredObject[]>>;
};

/** Where a chain stood when it was anchored. */
export type ChainHead = {
  /** The writer's chain id. */
  chain: string;
  /** The position of its newest stored row. */
  seq: number;
  /** That row's hash. */
  hash: string;
};

/** A chain head with the seal that shows this deployment wrote it. */
export type Anchor = ChainHead & {
  /** HMAC of the head under the chain key. */
  sig: string;
};

/** What comparing the anchors with the database found. */
export type AnchorVerdict = {
  /** True when every anchored row is still stored as it was anchored. */
  ok: boolean;
  /** How many chains had an anchor to compare. */
  anchors: number;
  /** The first anchored chain that does not match, and why. */
  broken?: AnchorBreak;
};

/** An anchored chain that no longer matches the database. */
export type AnchorBreak = ChainHead & {
  /** What did not match, in words an auditor can act on. */
  reason: string;
};

/** Looks up one stored audit row by its chain and position, or null when there is none. */
export type RowAt = (chain: string, seq: number) => Promise<Record<string, any> | null>;

/** The text an anchor seals: what it vouches for and nothing else. */
const sealed = ({ chain, seq, hash }: ChainHead) => `audit-anchor\n${chain}\n${seq}\n${hash}`;

/** The day folder an anchor taken at `at` is filed under. */
const dayOf = (at: Date) => at.toISOString().slice(0, ISO_DATE_CHARS);

/** An anchor's object name: chain, padded position, hash and seal. */
const objectName = (a: Anchor) =>
  `${a.chain}_${String(a.seq).padStart(AUDIT_ANCHOR_SEQ_DIGITS, '0')}_${a.hash}_${a.sig}.json`;

/** An anchor read back from its object name, or null for anything else in the bucket. */
function parseName(name: string): Anchor | null {
  const match = /^([a-z0-9-]+)_(\d+)_([0-9a-f]{64})_([0-9a-f]{64})\.json$/.exec(name);
  return match ? { chain: match[1], seq: Number(match[2]), hash: match[3], sig: match[4] } : null;
}

/** Seals a chain head and adds it to the bucket; throws when the bucket refuses it. */
export async function writeAnchor(objects: AnchorObjects, head: ChainHead, at = new Date()): Promise<Anchor> {
  const anchor = { chain: head.chain, seq: head.seq, hash: head.hash, sig: seal(sealed(head)) };
  const body = JSON.stringify({ ...anchor, at: at.toISOString() });
  const options = { contentType: 'application/json', upsert: false };
  const { error } = await objects.upload(`${dayOf(at)}/${objectName(anchor)}`, body, options);
  if (error) throw new Error(`audit anchor not written: ${error.message}`);
  return anchor;
}

/** Object names in one day's folder; throws when the bucket cannot be read. */
async function listDay(objects: AnchorObjects, day: string): Promise<string[]> {
  const { data, error } = await objects.list(day, { limit: AUDIT_ANCHOR_LIST_MAX });
  if (error) throw new Error(`audit anchors unreadable: ${error.message}`);
  return (data || []).map((o) => o.name);
}

/** The highest-positioned anchor of each chain. */
function newestPerChain(anchors: Anchor[]): Anchor[] {
  const newest = new Map<string, Anchor>();
  for (const a of anchors) if (!(newest.get(a.chain)?.seq >= a.seq)) newest.set(a.chain, a);
  return [...newest.values()];
}

/**
 * The newest anchor of every chain anchored in the last AUDIT_ANCHOR_VERIFY_DAYS.
 * ponytail: a chain whose last anchor is older is not compared; page through
 * older days, or keep a per-chain index object, if that window is too short.
 */
export async function latestAnchors(objects: AnchorObjects, now = Date.now()): Promise<Anchor[]> {
  const days = Array.from({ length: AUDIT_ANCHOR_VERIFY_DAYS }, (_, i) => dayOf(new Date(now - i * AUDIT_DAY_MS)));
  const names = (await Promise.all(days.map((day) => listDay(objects, day)))).flat();
  return newestPerChain(names.map(parseName).filter(Boolean));
}

/** Why an anchored chain no longer matches the database, or null when it does. */
async function anchorBreak(a: Anchor, rowAt: RowAt): Promise<string | null> {
  if (!sealMatches(sealed(a), a.sig)) return 'anchor seal does not match, it was not written by this deployment';
  const [anchored, first] = await Promise.all([rowAt(a.chain, a.seq), rowAt(a.chain, 1)]);
  if (!anchored) return `anchored row is missing, rows up to seq ${a.seq} were deleted`;
  if (anchored.hash !== a.hash) return 'anchored row differs from its anchor, the chain was rewritten';
  if (!first) return 'the start of the chain is missing, rows were deleted';
  return null;
}

/** Compares each anchor with the stored row it vouches for; the first mismatch wins. */
export async function checkAnchors(anchors: Anchor[], rowAt: RowAt): Promise<AnchorVerdict> {
  const reasons = await Promise.all(anchors.map((a) => anchorBreak(a, rowAt)));
  const index = reasons.findIndex(Boolean);
  if (index < 0) return { ok: true, anchors: anchors.length };
  const { chain, seq, hash } = anchors[index];
  return { ok: false, anchors: anchors.length, broken: { chain, seq, hash, reason: reasons[index] } };
}
