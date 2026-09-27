/**
 * REST routes for a person's plan: see it, subscribe, manage it, and the
 * webhook Stripe reports changes on. Signed-in people only; a self-hosted
 * server, which has no plans, says so and nothing more.
 */
import { Router } from 'express';
import { userAuthMiddleware } from '../auth/service.ts';
import { Status } from '../../platform/http-status.ts';
import { hosted, webhookSecret } from './config.ts';
import { verifySignature } from './stripe.ts';
import { notePayment } from './payments.ts';
import type { Billing } from './index.ts';

/** The routes, over the container's billing. */
export function billingRoutes(billing: Billing) {
  const router = Router({ caseSensitive: true });
  planRoutes(router, billing);
  invoiceRoutes(router, billing);
  return router;
}

/** The plan: see it, subscribe, manage it. */
function planRoutes(router, billing: Billing) {
  /** GET /billing, the person's plan, allowances and use this period; `{enabled: false}` when self-hosted. */
  router.get('/billing', userAuthMiddleware, async (req, res) => res.json(await summary(billing, req.user.id)));
  /** POST /billing/checkout {plan}, a Stripe Checkout page to subscribe on. */
  router.post('/billing/checkout', userAuthMiddleware, async (req, res) => res.json(await checkout(billing, req)));
  /** POST /billing/portal, the Stripe page to change card, see invoices, or cancel. */
  router.post('/billing/portal', userAuthMiddleware, async (req, res) =>
    res.json(await billing.subscriptions.portal(req.user.id)),
  );
}

/** Invoices: those issued, and the next one as it stands. */
function invoiceRoutes(router, billing: Billing) {
  /** GET /billing/invoices, the person's issued invoices, newest first. */
  router.get('/billing/invoices', userAuthMiddleware, async (req, res) =>
    res.json(await billing.invoices.list(req.user.id)),
  );
  /** GET /billing/upcoming, the next invoice as it stands, line by line. */
  router.get('/billing/upcoming', userAuthMiddleware, async (req, res) =>
    res.json(await billing.invoices.upcoming(req.user.id)),
  );
}

/** A Checkout page for the plan the request names. */
const checkout = (billing: Billing, req) => billing.subscriptions.checkout(req.user.id, String(req.body?.plan || ''));

/** The person's plan and use, or that this server has no plans. */
const summary = async (billing: Billing, userId: string) =>
  hosted() ? billing.subscriptions.summary(userId) : { enabled: false };

/**
 * POST /api/billing/webhook, Stripe's report of a subscription change. Mounted
 * ahead of the JSON parser: the signature is over the raw bytes.
 */
export function billingWebhook(billing: Billing) {
  return async (req, res) => {
    if (!hosted() || !verifySignature(req.body, String(req.headers['stripe-signature'] || ''), webhookSecret()))
      return res.status(Status.BAD_REQUEST).json({ error: 'Bad signature', code: 'invalid_request' });
    const event = JSON.parse(req.body.toString('utf8'));
    await billing.subscriptions.applyEvent(event);
    notePayment(event);
    res.json({ received: true });
  };
}
