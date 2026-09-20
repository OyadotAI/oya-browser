/**
 * The webhooks panel. One endpoint per project, Stripe-style: a URL, the
 * events it wants, and a signing secret shown only when it is minted. It saves
 * itself (see settings/use-webhook.ts); the parts live in settings/.
 */
'use client';

import type { RowComponent } from './settings/fields';
import { ErrorNote, Loading, SectionHeading } from './settings/heading';
import { useWebhook } from './settings/use-webhook';
import { Deliveries, SigningSecret, VerifyHelp, WebhookActions, WebhookFields } from './settings/webhook-parts';

/** The panel. */
export default function WebhookSection({
  apiKey,
  Row,
}: {
  /** The key whose webhook is configured. */
  apiKey: string;
  /** The dialog's row component. */
  Row: RowComponent;
}) {
  const webhook = useWebhook(apiKey);
  const { config, error, secret } = webhook;
  if (!config) return error ? <ErrorNote spaced={false}>{error}</ErrorNote> : <Loading>Loading your webhook…</Loading>;
  return (
    <>
      <SectionHeading eyebrow="Webhooks" title="Events, pushed to you.">
        One HTTPS endpoint receives the events you choose, signed so you can verify they came from Oya. Failed
        deliveries retry with backoff for a day.
      </SectionHeading>
      {error && <ErrorNote>{error}</ErrorNote>}
      <WebhookFields webhook={webhook} config={config} Row={Row} />
      <WebhookActions webhook={webhook} config={config} />
      {secret && <SigningSecret secret={secret} />}
      <VerifyHelp />
      {config.hook && <Deliveries webhook={webhook} config={config} />}
    </>
  );
}
