/**
 * Project operations: the durable session inventory, access controls,
 * budgets and event delivery for the key's project. Its pieces live in
 * ./control/ (durable-*.tsx, use-durable.ts).
 */
'use client';

import { DurableHeader, DurableNotices, DurableSummary } from './control/durable-overview';
import SessionInventory from './control/session-inventory';
import DurableSettings from './control/durable-settings';
import DurableCredentials from './control/durable-credentials';
import DurableWebhooks from './control/durable-webhooks';
import DurableActivity from './control/durable-activity';
import { useDurable } from './control/use-durable';

/** Loading or error until the first overview, then every section. */
export default function DurableControl({ apiKey }: { /** The connected API key. */ apiKey: string }) {
  const d = useDurable(apiKey);
  const data = d.data;
  if (!data)
    return (
      <div className="p-6 text-sm text-text-muted" role="status">
        {d.error || 'Loading durable session inventory…'}
      </div>
    );
  return (
    <div className="space-y-8 p-5 lg:p-8">
      <DurableHeader data={data} />
      <DurableNotices d={d} />
      <DurableSummary data={data} />
      <SessionInventory d={d} sessions={data.sessions} />
      {data.credentials && d.drafts.settings && (
        <section className="grid gap-8 xl:grid-cols-2">
          <DurableSettings d={d} />
          <DurableCredentials d={d} credentials={data.credentials} />
        </section>
      )}
      {data.webhooks && <DurableWebhooks d={d} webhooks={data.webhooks} deliveries={data.deliveries} />}
      <DurableActivity events={data.events} />
    </div>
  );
}
