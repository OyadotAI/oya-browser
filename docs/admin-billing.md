# Admin plan access and usage grants

In **Admin → Customers**, look up the customer's email. Their customer panel
contains **Plan access** and **Extra allowance**. Both actions require a reason
and the existing administrator authentication (including MFA when configured).

- **Plan access** sets Free, Developer or Startup access until an administrator
  chooses **Follow subscription**. This is an access override; it does not create,
  change or cancel a Stripe subscription or its charges. Complimentary access
  stops at the selected plan's included allowances. Hosted AI credit can be
  granted separately. Stripe webhooks cannot overwrite the override.
- **Extra allowance** grants cloud hours and hosted AI cost in USD for the
  customer's current billing period. Multiple grants add together. Unused grants
  expire at the next period; they do not carry forward. Raw usage is preserved.
  Grants extend admission limits and reduce usage not yet reported to Stripe.
  Already reported usage and existing invoices are not refunded or reversed.
- The panel lists grants for the current period with amounts, dates and reasons.
  Grants persist with the issuing administrator. The audit log records both plan
  changes and grants. Retrying the same grant request id cannot issue it twice.

Deploy `server/migrations/022_admin_billing.sql` through the normal migration
runner before starting the updated server on Postgres/Supabase. Hosted dev and production workflows apply this additive migration before rollout.
SQLite creates the new tables on startup. No live billing changes are made by the migration.

API: `PUT /api/admin/users/:id/plan` accepts `{plan, reason}` (`plan: null`
restores the subscription). `POST /api/admin/users/:id/grants` accepts
`{requestId, hours, credits, reason}`; `requestId` must be a UUID v4, and at least
one amount must be positive. Reuse a request id only for an identical retry.
