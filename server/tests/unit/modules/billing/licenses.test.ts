/**
 * Unit tests for selling self-hosted licenses: Checkout per browser, one key
 * per paid invoice however often Stripe delivers it, and the key shown only to
 * the person who bought it.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { LicenseSales } from '../../../../src/modules/billing/licenses.ts';

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);
/** Seconds for 10 April 2026, the paid period's end. */
const PERIOD_END = Date.UTC(2026, 3, 10) / 1000;
/** Fourteen days of grace, in seconds. */
const GRACE = 14 * 86400;

/** License sales over rows in memory, a Stripe that records its calls, and a minter that counts. */
function fixture(subscription: any = null) {
  const rows: any[] = [];
  const posts: any[] = [];
  const minted: any[] = [];
  const stripe = {
    post: async (path, params) => (posts.push({ path, params }), { url: `https://stripe.test${path}` }),
    get: async () => (subscription ? subscription : Promise.reject(new Error('none'))),
  };
  const deps = {
    stripe,
    openCheckout: async (params) => stripe.post('/checkout/sessions', params),
    mint: (req) => (minted.push(req), { license: { id: `L${minted.length}`, ...req }, token: `KEY${minted.length}` }),
    seal: (scope, value) => `${scope}|${value}`,
    open: (scope, sealed) => {
      const [s, value] = sealed.split('|');
      if (s !== scope) throw new Error('wrong scope');
      return value;
    },
    byInvoice: async (id) => rows.find((r) => r.stripe_invoice_id === id) ?? null,
    forUser: async (id) => rows.filter((r) => r.user_id === id && !r.revoked_at).reverse(),
    insert: async (row) => void rows.push(row),
    revokedUnder: async (sub) => rows.some((r) => r.stripe_subscription_id === sub && r.revoked_at),
    revokeUnder: async (sub, at) =>
      rows.filter((r) => r.stripe_subscription_id === sub && !r.revoked_at).map((r) => (r.revoked_at = at)).length,
    returnUrl: () => 'https://console.test/dashboard/billing',
    now: () => NOW,
  };
  return { sales: new LicenseSales(deps as any), rows, posts, minted };
}

/** A paid invoice for `quantity` browsers on `price`, for user u1, in API 2025-03-31's shape. */
const paid = (id = 'in_1', quantity = 12, price = 'license_month', kind = 'self_hosted_license') => ({
  type: 'invoice.paid',
  data: {
    object: {
      id,
      customer: 'cus_1',
      customer_email: 'ops@acme.test',
      parent: { subscription_details: { subscription: 'sub_L', metadata: { user_id: 'u1', kind } } },
      lines: { data: [{ quantity, period: { end: PERIOD_END }, pricing: { price_details: { price } } }] },
    },
  },
});

describe('LicenseSales', () => {
  beforeEach(() =>
    Object.assign(process.env, {
      STRIPE_PRICE_LICENSE_MONTH: 'license_month',
      STRIPE_PRICE_LICENSE_YEAR: 'license_year',
    }),
  );
  afterEach(() => ['STRIPE_PRICE_LICENSE_MONTH', 'STRIPE_PRICE_LICENSE_YEAR'].forEach((n) => delete process.env[n]));

  it('opens Checkout for the browsers asked, per browser, tagged as a license so the plan webhook ignores it', async () => {
    const { sales, posts } = fixture();
    assert.deepEqual(await sales.checkout('u1', 12, 'year'), { url: 'https://stripe.test/checkout/sessions' });
    const { params } = posts[0];
    assert.deepEqual(params.line_items, [{ price: 'license_year', quantity: 12 }]);
    assert.deepEqual(params.subscription_data.metadata, { user_id: 'u1', kind: 'self_hosted_license' });
    assert.equal(params.success_url, 'https://console.test/dashboard/billing?license=paid#self-hosted');
  });

  it('refuses fewer than 6 browsers, more than 50, and part of one', async () => {
    const { sales, posts } = fixture();
    for (const n of [5, 51, 6.5, 'six'])
      await assert.rejects(sales.checkout('u1', n, 'month'), { status: 400, field: 'browsers' });
    assert.equal(posts.length, 0);
  });

  it('refuses a billing period other than month or year', async () => {
    await assert.rejects(fixture().sales.checkout('u1', 6, 'week'), { status: 400, field: 'interval' });
  });

  it('answers 503 while the license prices are not set', async () => {
    delete process.env.STRIPE_PRICE_LICENSE_MONTH;
    await assert.rejects(fixture().sales.checkout('u1', 6, 'month'), { status: 503 });
  });

  it('refuses a second purchase while a license is still being paid for', async () => {
    const { sales, posts } = fixture({ status: 'active', cancel_at_period_end: false });
    await sales.applyEvent(paid());
    await assert.rejects(sales.checkout('u1', 6, 'month'), { status: 409, code: 'license_exists' });
    assert.equal(posts.length, 0);
  });

  it('sells again once the license is set to end, to the same Stripe customer', async () => {
    const { sales, posts } = fixture({ status: 'active', cancel_at_period_end: true });
    await sales.applyEvent(paid());
    await sales.checkout('u1', 6, 'month');
    assert.equal(posts[0].params.customer, 'cus_1');
  });

  it('issues one key for a paid invoice, for its browsers, good until its period ends plus the grace', async () => {
    const { sales, rows, minted } = fixture();
    await sales.applyEvent(paid());
    const expiresAt = new Date((PERIOD_END + GRACE) * 1000).toISOString();
    assert.deepEqual(minted, [{ licensee: 'ops@acme.test', maxConcurrent: 12, expiresAt }]);
    assert.equal(rows[0].created_by, 'stripe');
    assert.equal(rows[0].interval, 'month');
    assert.equal(rows[0].stripe_subscription_id, 'sub_L');
    assert.equal(rows[0].key_sealed, 'license:u1|KEY1');
  });

  it('issues nothing more when Stripe delivers the same invoice again', async () => {
    const { sales, rows, minted } = fixture();
    await sales.applyEvent(paid());
    await sales.applyEvent(paid());
    assert.equal(rows.length, 1);
    assert.equal(minted.length, 1);
  });

  it('ignores plan invoices, other prices and other events', async () => {
    const { sales, rows } = fixture();
    await sales.applyEvent(paid('in_2', 1, 'base_dev', undefined));
    await sales.applyEvent({ ...paid('in_4'), type: 'invoice.payment_failed' });
    await sales.applyEvent({ type: 'invoice.paid' });
    assert.equal(rows.length, 0);
  });

  it('after a quantity change issues the key for the new period, not the proration line for the old one', async () => {
    const { sales, minted } = fixture();
    const event = paid('in_1', 20);
    const old = {
      quantity: 12,
      period: { end: PERIOD_END - 86400 },
      pricing: { price_details: { price: 'license_month' } },
    };
    event.data.object.lines.data.unshift(old as any);
    await sales.applyEvent(event);
    assert.equal(minted[0].maxConcurrent, 20);
  });

  it('warns when a paid license invoice carries no current license price, as after a price rotation', async (t) => {
    const warn = t.mock.method(console, 'warn', () => {});
    const { sales, rows } = fixture();
    await sales.applyEvent(paid('in_1', 12, 'retired_price'));
    assert.equal(rows.length, 0);
    assert.match(String(warn.mock.calls[0].arguments[0]), /in_1 has no current license price/);
  });

  it('issues no new key on a subscription whose key was revoked, ends its billing, and says to refund', async (t) => {
    const warn = t.mock.method(console, 'warn', () => {});
    const { sales, rows, posts } = fixture();
    await sales.applyEvent(paid('in_1'));
    rows[0].revoked_at = '2026-03-20T00:00:00.000Z';
    await sales.applyEvent(paid('in_2'));
    assert.equal(rows.length, 1);
    assert.deepEqual(posts, [{ path: '/subscriptions/sub_L', params: { cancel_at_period_end: true } }]);
    assert.match(String(warn.mock.calls[0].arguments[0]), /in_2 paid on a revoked subscription, refund it by hand/);
  });

  it('reads the line price from before API 2025-03-31 too', async () => {
    const { sales, rows } = fixture();
    const event = paid();
    event.data.object.lines.data[0] = {
      quantity: 7,
      period: { end: PERIOD_END },
      price: { id: 'license_year' },
    } as any;
    await sales.applyEvent(event);
    assert.equal(rows[0].interval, 'year');
  });

  it('stores nothing when issuing the key fails, so Stripe retries the delivery', async () => {
    const { sales, rows } = fixture();
    (sales.deps as any).mint = () => {
      throw new Error('no signing key');
    };
    await assert.rejects(sales.applyEvent(paid()));
    assert.equal(rows.length, 0);
  });

  it('shows the buyer their key, browsers and renewal', async () => {
    const sub = {
      status: 'active',
      cancel_at_period_end: false,
      items: { data: [{ current_period_end: PERIOD_END }] },
    };
    const { sales } = fixture(sub);
    await sales.applyEvent(paid());
    const { license } = (await sales.view('u1')) as any;
    assert.equal(license.key, 'KEY1');
    assert.equal(license.browsers, 12);
    assert.equal(license.status, 'active');
    assert.equal(license.renewsAt, new Date(PERIOD_END * 1000).toISOString());
    assert.equal(license.previousExpiresAt, null);
  });

  it('shows nobody else’s key', async () => {
    const { sales } = fixture();
    await sales.applyEvent(paid());
    assert.deepEqual(await sales.view('u2'), { enabled: true, license: null });
  });

  it('after a renewal shows the new key and when the old one stops', async () => {
    const { sales } = fixture();
    await sales.applyEvent(paid('in_1'));
    await sales.applyEvent(paid('in_2'));
    const { license } = (await sales.view('u1')) as any;
    assert.equal(license.key, 'KEY2');
    assert.equal(license.previousExpiresAt, new Date((PERIOD_END + GRACE) * 1000).toISOString());
  });

  it('shows no renewal once the license is set to end, and still shows the key when Stripe cannot be reached', async () => {
    const { sales } = fixture({ status: 'active', cancel_at_period_end: true, current_period_end: PERIOD_END });
    await sales.applyEvent(paid());
    const { license } = (await sales.view('u1')) as any;
    assert.equal(license.renewsAt, null);
    assert.equal(license.cancelAtPeriodEnd, true);
    const offline = fixture();
    await offline.sales.applyEvent(paid());
    assert.equal(((await offline.sales.view('u1')) as any).license.key, 'KEY1');
  });

  it('opens the Portal for the license’s customer, and 404s without a license', async () => {
    const { sales, posts } = fixture();
    await assert.rejects(sales.portal('u1'), { status: 404 });
    await sales.applyEvent(paid());
    assert.deepEqual(await sales.portal('u1'), { url: 'https://stripe.test/billing_portal/sessions' });
    assert.equal(posts[0].params.customer, 'cus_1');
    assert.equal(posts[0].params.return_url, 'https://console.test/dashboard/billing#self-hosted');
  });
});
