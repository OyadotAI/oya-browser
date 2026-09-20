/**
 * Where a new provider connects: a CDP WebSocket URL for your own endpoint,
 * or an API key for a hosted vendor (optional when one is already saved).
 */
import type { Control } from './use-control';

/** The CDP WebSocket URL field, with where to find it. */
export function CdpUrlField({ ctl }: { /** Control state and actions. */ ctl: Control }) {
  return (
    <label className="text-xs text-text-dim space-y-1 block">
      <span>CDP WebSocket URL</span>
      <input
        required
        disabled={!!ctl.busy}
        value={ctl.draft.wsUrl}
        onChange={(e) => ctl.edit({ wsUrl: e.target.value })}
        placeholder="ws://127.0.0.1:9222/devtools/browser/…"
        className="settings-input font-mono"
      />
      <span className="block text-text-dim">
        From <code className="font-mono">http://host:9222/json/version</code> →{' '}
        <code className="font-mono">webSocketDebuggerUrl</code>. Private and loopback addresses need{' '}
        <code className="font-mono">OYA_ALLOW_PRIVATE_TARGETS=true</code> on the host.
      </span>
    </label>
  );
}

/** The vendor API key field; blank keeps a credential already saved for this vendor. */
export function VendorKeyField({ ctl }: { /** Control state and actions. */ ctl: Control }) {
  const saved = ctl.choices.some((p) => p.name === ctl.draft.type && p.configured);
  return (
    <label className="text-xs text-text-muted space-y-2 block">
      <span>Provider API key</span>
      <input
        type="password"
        autoComplete="new-password"
        spellCheck={false}
        disabled={!!ctl.busy}
        required={!saved}
        value={ctl.draft.apiKey}
        onChange={(e) => ctl.edit({ apiKey: e.target.value })}
        placeholder={saved ? 'Use saved credential' : 'Paste your provider API key'}
        className="settings-input font-mono"
      />
      <span className="block leading-5">
        {saved
          ? 'Leave blank to use the saved credential. Pasting a key replaces this vendor’s credential for your Oya key.'
          : 'Saved securely for your Oya key and also available in Settings → Browsers.'}{' '}
        Each CDP connection starts a browser with this vendor.
      </span>
    </label>
  );
}
