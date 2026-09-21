/**
 * The proxies dialog: the exits profiles can use. Its table, form and actions
 * live in `personas/`.
 */
'use client';

import { Loader2, RefreshCw } from 'lucide-react';
import Dialog from '@/components/ui/dialog';
import { check } from './personas/proxy-actions';
import ProxyForm from './personas/proxy-form';
import ProxyTable from './personas/proxy-table';
import { useProxies, type ProxiesProps } from './personas/use-proxies';

export type { ProxyRow } from './personas/model';

/**
 * The exits profiles can use. A proxy added here is picked by a profile's geo
 * hint at first connect, or pinned from the profile drawer, and then kept.
 */
export default function ProxiesDialog(props: ProxiesProps) {
  const s = useProxies(props);
  return (
    <Dialog
      open={props.open}
      onClose={props.onClose}
      title="Proxies"
      size="lg"
      description="Exits your profiles use. Each profile keeps the proxy it is given, because an IP that changes looks like a stolen login."
      footer={
        <button className="btn-ghost" onClick={props.onClose}>
          Done
        </button>
      }
    >
      <div className="space-y-5">
        <ProxyTable s={s} />
        {s.rows.length > 0 && (
          <button className="btn-ghost h-8" onClick={() => check(s)} disabled={s.busy === 'check'}>
            {s.busy === 'check' ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}{' '}
            Check all
          </button>
        )}
        <ProxyForm s={s} />
      </div>
    </Dialog>
  );
}
