# AWS Marketplace fulfillment

Fulfillment setup page: **`https://oyabrowser.com/marketplace/aws`**.

**This change implements account-setup preparation, not a production-ready
fulfillment integration. Serving this page does not activate AWS subscriptions.
Do not submit it as a working fulfillment URL until the activation work below is done.**

AWS posts `x-amzn-marketplace-token` as a form field to the seller's fulfillment
URL. Oya immediately calls ResolveCustomer using the seller's IAM identity,
checks the configured ProductCode, and seals the returned CustomerAWSAccountId
and LicenseArn in a short-lived HttpOnly cookie. New integrations must use these
identifiers rather than assume a legacy CustomerIdentifier is returned.
[AWS account setup](https://docs.aws.amazon.com/marketplace/latest/userguide/saas-product-customer-setup.html),
[ResolveCustomer](https://docs.aws.amazon.com/marketplace/latest/APIReference/API_marketplace-metering_ResolveCustomer.html).

Buyers then create an Oya account or sign in through the existing account,
CAPTCHA, OAuth and MFA flows. They explicitly choose “Connect to this account”.
The server records immutable AWS account ownership and a separate license
registration. Repeated requests cannot transfer that buyer to another Oya
account. These records survive session expiry and remain as billing ownership
tombstones if an Oya account is deleted. Support must verify ownership before
any manual correction; deleting the rows is not an account-recovery procedure.

The page says **Waiting for AWS confirmation** after linking. Neither successful
token resolution nor a stored registration creates an active subscription, a
paid plan, a Stripe subscription or an administrator access override. There is
no environment flag or public event endpoint that can turn pending into
confirmed in this preparation.

## Local and deployment configuration

Registration is disabled by default. For isolated setup testing, configure:

```dotenv
OYA_AWS_MARKETPLACE_ENABLED=true
OYA_AWS_MARKETPLACE_PRODUCT_CODE=<actual seller product code>
OYA_AWS_MARKETPLACE_REGION=us-east-1
OYA_CONSOLE_URL=https://oyabrowser.com
OYA_PROFILE_SECRET=<existing shared secret from the deployment secret manager>
```

Use the existing shared profile-encryption secret; never rotate it just to
enable Marketplace, because other encrypted Oya records use it too. All replicas
need the same secret and salt settings. A generated local disk key is refused
for this feature. Public URLs must use HTTPS; HTTP is accepted only for explicit
loopback development. The UI, API and AWS POST entry point must share an origin.

The server's AWS SDK default credential chain must resolve to the seller account
that published the product, with `aws-marketplace:ResolveCustomer`. Prefer an
IAM task/instance role. Do not put access keys in the frontend or commit them.
Apply `server/migrations/027_aws_marketplace.sql` before rolling out the server.
Postgres provides cross-replica ownership. SQLite and file storage keep their
existing single-process scope. Rollback leaves the additive tables in place;
retain buyer ownership records.

The AWS POST body is limited to 64 KiB, with a small field count and an opaque
token limit. The exchange has an eight-second caller deadline, two SDK attempts
and four outstanding SDK operations per process; an ignored abort retains its
slot until underlying work settles. SDK errors are redacted. Callback failures
clear earlier purchase context and redirect using only a fixed error name.
An Oya setup session lasts thirty minutes; this is not an AWS token-expiry
promise. Original AWS tokens are never saved in a cookie, URL or database.

`GET /api/marketplace/aws` returns anonymous resolved context with only the last
four AWS account digits. Account-authenticated `GET .../status` reads registration.
`POST .../connect` requires a complete account session, same-origin request,
an empty JSON object and an `Idempotency-Key`. Administrative impersonation is
refused. Expired contexts require a fresh AWS handoff, preserving durable
ownership. No GET request or auth restore links a purchase.

Context/status reads return an opaque context nonce. An explicit connection must
send that same value in `X-Oya-Marketplace-Context`; replacing the shared cookie
from another tab invalidates the old consent before any write. The buyer must
review the new purchase and explicitly connect it. This binding was added after
review reproduced a cross-tab cookie replacement; a host-wide cookie alone was
insufficient to identify what the buyer had agreed to connect.

## Required work before activation

Confirm the listing's product code, pricing model, seller event setup and the
Oya access/entitlement mapping. No such configuration was found in this checkout.

Current AWS onboarding requires a **License Updated** event before activation.
Sellers registered after July 15, 2026 must use EventBridge. Implement the
applicable authenticated event delivery, durable processing, ordering,
reconciliation and revocation handling from verified event schemas. The event
payload/ordering contract has not been verified here and must not be guessed.
[AWS onboarding](https://docs.aws.amazon.com/marketplace/latest/userguide/saas-product-customer-setup.html),
[SaaS EventBridge integration](https://docs.aws.amazon.com/marketplace/latest/userguide/saas-eventbridge-integration.html).

For a contract listing, verify entitlements in us-east-1 with trusted account,
license and product filters; follow every NextToken even after an empty page.
Map only explicitly agreed dimensions and quantities to Oya access. For a
usage-based component, implement the required usage and final reporting as well.
AWS access must be read through a separate admission seam, never written into
Stripe-owned rows or reported to Stripe.
[GetEntitlements](https://docs.aws.amazon.com/marketplace/latest/APIReference/API_marketplace-entitlements_GetEntitlements.html),
[Checking entitlements](https://docs.aws.amazon.com/marketplace/latest/userguide/checking-entitlements.html).

Before launch, verify a real buyer POST, new and existing account authentication,
MFA, active/pending/refused/expired purchases, duplicate delivery, another-account
conflicts, revocation and applicable metering in the seller's test environment.
Local hermetic fixtures are not proof of a real AWS subscription.

## Preparation limits and deliberate deviations

The principal's design explicitly permits this independent registration work
while authoritative activation is blocked. Full subscription confirmation and
the designer's success state remain unimplemented. No plan or listing type was
invented, and the integration stays disabled by default.

The immutable buyer/license rows provide durable domain idempotency: retries by
the same owner converge on the same safe pending state, including after a partial
write. The header is required, but its request hash and serialized response are
not persisted. This is a deliberate deviation from the production checklist's
general Idempotency-Key storage rule for this disabled preparation; resolve it
before exposing a paid fulfillment flow. Existing ingress admission buckets are
per-process; shared/ingress rate enforcement also remains a production dependency.
