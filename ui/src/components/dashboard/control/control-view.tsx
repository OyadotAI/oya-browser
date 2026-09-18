/**
 * Picks the body of the current Control view. A command map from view to
 * renderer, so adding a view is one entry, not another branch.
 */
import type { ReactNode } from 'react';
import DurableControl from '../durable-control';
import AuditView from './audit-view';
import HealthView from './health-view';
import ProvidersView from './providers-view';
import RecordingsView from './recordings-view';
import SessionsView from './sessions-view';
import UsageView from './usage-view';
import type { Control } from './use-control';
import type { View } from './types';

/** What every view renderer is given. */
interface ViewProps {
  /** Control state and actions. */
  ctl: Control;
  /** Opens the recording player. */
  onPlay: (sessionId: string) => void;
}

/** View → its body. Providers waits for routing to load. */
const RENDERERS: Record<View, (props: ViewProps) => ReactNode> = {
  operations: ({ ctl }) => <DurableControl apiKey={ctl.apiKey} />,
  health: ({ ctl }) => <HealthView fleet={ctl.fleet} />,
  sessions: ({ ctl }) => <SessionsView ctl={ctl} />,
  providers: ({ ctl }) => ctl.routing && <ProvidersView ctl={ctl} routing={ctl.routing} />,
  usage: ({ ctl }) => <UsageView usage={ctl.fleet?.usage} />,
  audit: ({ ctl }) => <AuditView audit={ctl.audit} />,
  recordings: ({ ctl, onPlay }) => <RecordingsView ctl={ctl} onPlay={onPlay} />,
};

/** The body of `view`, or nothing for an unknown one. */
export default function ControlView({ view, ...props }: ViewProps & { /** The view on screen. */ view: View }) {
  return Object.hasOwn(RENDERERS, view) ? RENDERERS[view](props) : null;
}
