/** Pagination bounds reject incomplete collections, late responses and repeated embedded identifiers. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { financialBudget, financialList, IncompleteFinancial } from '../../../../src/modules/admin/financial-pages.ts';

/** Fake collections never call a network. */
const stripe = (read: () => any) => ({ post: async () => ({}), get: async () => read() });

it('rejects an expired metric before making a request', async () => {
  let now = 0,
    calls = 0;
  const budget = financialBudget(new AbortController().signal, () => now);
  now = 10000;
  await assert.rejects(
    financialList(
      stripe(() => {
        calls++;
      }),
      '/invoices',
      {},
      budget,
    ),
    IncompleteFinancial,
  );
  assert.equal(calls, 0);
});

it('rejects a response arriving after its snapshot deadline', async () => {
  let now = 0;
  const budget = financialBudget(new AbortController().signal, () => now);
  await assert.rejects(
    financialList(
      stripe(() => {
        now = 10001;
        return { data: [], has_more: false };
      }),
      '/invoices',
      {},
      budget,
    ),
    IncompleteFinancial,
  );
});

it('never turns a missing collection or empty continuation into a complete zero', async () => {
  for (const collection of [{}, { data: [] }, { data: [], has_more: true }])
    await assert.rejects(
      financialList(
        stripe(() => collection),
        '/invoices',
        {},
        financialBudget(),
      ),
      IncompleteFinancial,
    );
});

it('shares the object limit with already processed nested records', async () => {
  const budget = financialBudget();
  budget.objects = 2000;
  await assert.rejects(
    financialList(
      stripe(() => ({ data: [{ id: 'one-more' }], has_more: false })),
      '/subscription_items',
      {},
      budget,
    ),
    IncompleteFinancial,
  );
});

it('rejects duplicate embedded identifiers before requesting another page', async () => {
  let calls = 0;
  await assert.rejects(
    financialList(
      stripe(() => {
        calls++;
      }),
      '/subscription_items',
      {},
      financialBudget(),
      [{ id: 'same' }, { id: 'same' }],
    ),
    IncompleteFinancial,
  );
  assert.equal(calls, 0);
});
