/**
 * The browser panel: one browser, close up. Its state lives in
 * `browser/use-browser-panel.ts`, its parts in `browser/`.
 */
'use client';

import type { RefObject } from 'react';
import Dialog from '@/components/ui/dialog';
import LiveView from './live-view';
import ActivityLog from './activity-log';
import PanelHeader from './browser/panel-header';
import ControlBar from './browser/control-bar';
import UrlBar from './browser/url-bar';
import PanelActions from './browser/panel-actions';
import Stats from './browser/stats';
import ElementsList from './browser/elements-list';
import { toggleControl } from './browser/control';
import { useBrowserPanel } from './browser/use-browser-panel';

/** What the page hands the panel. */
interface Props {
  /** The project key. */
  apiKey: string;
  /** The browser shown. */
  browserId: string;
  /** Closes the panel. */
  onClose: () => void;
  /** Stops the given browsers. */
  onStop: (ids: string[]) => void;
  /** Opens a persona in its drawer. */
  onOpenPersona: (id: string) => void;
  /** Opens the connect dialog. */
  onConnect: (id: string) => void;
  /** Lets the page's shortcut focus the URL bar. */
  urlRef: RefObject<HTMLInputElement | null>;
  /** Clock the relative times are measured from. */
  now: number;
}

/**
 * One browser, close up: what it is looking at, what it has been doing, and a
 * way to act on it. Polled every 2s while open.
 */
export default function BrowserPanel(props: Props) {
  const { urlRef, now } = props;
  const s = useBrowserPanel(props);
  const { detail: d, ctx, busy, controlMode, live, inspect } = s;
  // Console input is human input; the server refuses it unless a person holds control.
  const human = controlMode === 'human';
  const needsControl = human ? undefined : 'Take control to drive this browser';
  const gate = { human, needsControl };
  return (
    <aside
      className="flex h-full w-full flex-col border-l border-border bg-bg-card lg:w-[540px]"
      aria-label="Browser detail"
    >
      <PanelHeader {...props} d={d} copied={s.copy.copied} copyId={s.copy.copyId} />
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-3">
        <ControlBar mode={controlMode} onToggle={() => toggleControl(ctx, controlMode, s.setControlMode)} />
        <UrlBar d={d} nav={s.nav} send={ctx.send} busy={busy} urlRef={urlRef} {...gate} />
        <div className="group">
          <LiveView
            frameSrc={live.frame}
            fps={live.fps}
            frameAgeMs={live.frameAt ? now - live.frameAt : null}
            send={ctx.send}
            onInput={s.onInput}
            interactive={human}
          />
        </div>
        <PanelActions ctx={ctx} busy={busy} screenshot={inspect.screenshot} analyze={inspect.analyze} {...gate} />
        {d && <Stats d={d} now={now} />}
        {inspect.elements && (
          <ElementsList
            elements={inspect.elements}
            onHide={() => inspect.setElements(null)}
            onInput={s.onInput}
            send={ctx.send}
            {...gate}
          />
        )}
        <ActivityLog items={d?.activity || []} optimistic={s.optimistic} now={now} />
      </div>
      <Dialog open={!!inspect.shot} onClose={() => inspect.setShot(null)} title="Screenshot" size="lg">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        {inspect.shot && <img src={inspect.shot} alt="Screenshot" className="w-full rounded-md border border-border" />}
      </Dialog>
    </aside>
  );
}
