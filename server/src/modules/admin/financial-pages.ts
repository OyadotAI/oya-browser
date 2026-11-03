/** Bounded cursor traversal shares request, object and abort budgets across nested collections. */
import type { Stripe } from '../billing/index.ts';
import {
  FINANCIAL_DEADLINE_MS,
  FINANCIAL_MAX_OBJECTS,
  FINANCIAL_MAX_REQUESTS,
  FINANCIAL_PAGE_SIZE,
} from './constants.ts';

/** Pagination or unsupported source data makes an exact total incomplete. */
export class IncompleteFinancial extends Error {}

/** One metric owns one deadline and all nested request/object counts. */
export interface FinancialBudget {
  /** Cancels real network requests. */ signal: AbortSignal;
  /** Absolute deadline. */ deadline: number;
  /** Clock seam for deterministic tests. */ now: () => number;
  /** Includes nested item requests. */ requests: number;
  /** Includes embedded objects. */ objects: number;
}

/** Independent metrics do not consume each other's budget or failure state. */
export function financialBudget(signal = AbortSignal.timeout(FINANCIAL_DEADLINE_MS), now = Date.now): FinancialBudget {
  return { signal, now, deadline: now() + FINANCIAL_DEADLINE_MS, requests: 0, objects: 0 };
}

/** Deadline checks also reject a response arriving after its snapshot budget. */
export function checkDeadline(budget: FinancialBudget) {
  if (budget.signal.aborted || budget.now() >= budget.deadline)
    throw new IncompleteFinancial('Financial read exceeded its time limit. Retry.');
}

/** Embedded and separately paginated items share the same memory/processing bound. */
export function countObjects(budget: FinancialBudget, count: number) {
  budget.objects += count;
  if (budget.objects > FINANCIAL_MAX_OBJECTS) throw new IncompleteFinancial('Financial read reached its object limit.');
}

/** Source collection shape must be complete before any row is counted. */
function validPage(page) {
  return (
    page &&
    Array.isArray(page.data) &&
    typeof page.has_more === 'boolean' &&
    page.data.every((row) => row && typeof row.id === 'string' && row.id)
  );
}

/** Missing collection metadata cannot silently look like an empty final page. */
export function checkedPage(page, budget: FinancialBudget): Record<string, any>[] {
  if (!validPage(page)) throw new IncompleteFinancial('Stripe returned an incomplete collection.');
  countObjects(budget, page.data.length);
  return page.data;
}

/** The original five-second client timeout is combined with the metric's abortable deadline. */
async function pageRequest(stripe: Stripe, path: string, query, budget: FinancialBudget) {
  checkDeadline(budget);
  if (budget.requests >= FINANCIAL_MAX_REQUESTS)
    throw new IncompleteFinancial('Financial read reached its request limit.');
  budget.requests += 1;
  const page = await stripe.get(path, { ...query, limit: FINANCIAL_PAGE_SIZE }, budget.signal);
  checkDeadline(budget);
  return page;
}

/** Each traversal owns its cursor, seen identifiers and bounded accumulated rows. */
interface Traversal {
  /** Accumulated source records. */ rows: Record<string, any>[];
  /** Prevents repeated records from inflating totals even when the final cursor advances. */ seen: Set<string>;
  /** Last processed source record. */ cursor?: string;
}

/** Every source identifier can contribute at most once to an authoritative total. */
function remember(rows: Record<string, any>[], seen: Set<string>) {
  for (const row of rows) {
    if (seen.has(row.id)) throw new IncompleteFinancial('Stripe returned repeated source identifiers.');
    seen.add(row.id);
  }
}

/** Duplicate or empty continuation pages make a total incomplete instead of double-counted. */
function append(page, budget: FinancialBudget, traversal: Traversal) {
  const rows = checkedPage(page, budget);
  remember(rows, traversal.seen);
  if (page.has_more && !rows.length) throw new IncompleteFinancial('Stripe returned an empty continuation page.');
  traversal.rows.push(...rows);
  traversal.cursor = rows.at(-1)?.id;
}

/** Nested items seed a traversal without charging their already-counted objects twice. */
function traversalFor(initial: Record<string, any>[]): Traversal {
  const seen = new Set<string>();
  remember(initial, seen);
  return { rows: [...initial], seen, cursor: initial.at(-1)?.id };
}

/** One traversal owns the client, immutable collection query and shared metric budget. */
interface Read {
  /** Authorized Stripe client. */ stripe: Stripe;
  /** Collection endpoint. */ path: string;
  /** Collection filters, never creation-time invoice cutoffs. */ query: Record<string, unknown>;
  /** Shared nested bound. */ budget: FinancialBudget;
}

/** Sequential requests cannot outlive their independent metric budget. */
async function walk(read: Read, traversal: Traversal) {
  do {
    const query = { ...read.query, ...(traversal.cursor && { starting_after: traversal.cursor }) };
    const page = await pageRequest(read.stripe, read.path, query, read.budget);
    append(page, read.budget, traversal);
    if (!page.has_more) return traversal.rows;
  } while (traversal.cursor);
  throw new IncompleteFinancial('Stripe pagination ended before a complete collection.');
}

/** Source records remain structurally compatible with the private Stripe seam. */
type Rows = Record<string, any>[];

/** Creation order never becomes a payment-date boundary; embedded items continue from their final ID. */
export async function financialList(stripe: Stripe, path: string, query, budget: FinancialBudget, initial: Rows = []) {
  return walk({ stripe, path, query, budget }, traversalFor(initial));
}
