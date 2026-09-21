/**
 * The Control tab: the key's operations, health, CDP sessions, providers,
 * usage, audit trail and recordings. Its views live in ./control/.
 */
'use client';

import { useState } from 'react';
import ControlBanner from './control/control-banner';
import ControlView from './control/control-view';
import Player from './control/player';
import ViewHeading from './control/view-heading';
import ViewTabs from './control/view-tabs';
import { useControl } from './control/use-control';
import type { View } from './control/types';

/** The Control workspace for the connected API key. */
export default function ControlTab({ apiKey }: { /** The connected API key. */ apiKey: string }) {
  const [view, setView] = useState<View>('health');
  const [player, setPlayer] = useState<string | null>(null);
  const ctl = useControl(apiKey);
  return (
    <div className="control-workspace flex min-w-0 flex-col h-full overflow-hidden">
      <ViewTabs view={view} onView={setView} onRefresh={() => void ctl.refresh()} />
      <ControlBanner error={ctl.actionError || ctl.error} notice={ctl.notice} />
      <div className="control-content min-w-0 flex-1 overflow-y-auto p-4 lg:p-8">
        <ViewHeading view={view} />
        <ControlView view={view} ctl={ctl} onPlay={setPlayer} />
      </div>
      {player && <Player sessionId={player} apiKey={apiKey} onClose={() => setPlayer(null)} />}
    </div>
  );
}
