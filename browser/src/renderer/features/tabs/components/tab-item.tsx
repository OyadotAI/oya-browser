/**
 * One tab on the strip: its icon (the page's favicon, a spinner while it
 * loads, the Oya mark on the start page, a globe otherwise), its title (the
 * role=tab button) and its close button, which a pinned tab, drawn as its
 * icon alone, does not have. The agent's tab carries a small Oya
 * light while the agent works, wherever the person is; a tab a call is
 * capturing carries a still red ring. A folding tab is inert
 * and no longer a tab; during a drag it slides by the drag's transform.
 */
import { useState, type RefObject } from 'react';
import { Icon, IconButton } from '../../../ui/index.ts';
import { closeLabel, iconKind, loadError, tabTitle, type TabItemModel } from '../model/tab-model.ts';
import { clickHandlers, pointerHandlers } from './tab-gestures.ts';
import { TEXT } from '../model/constants.ts';
import type { TabStripViewModel } from '../view-models/tab-strip-view-model.ts';
import type { TabDragViewModel } from '../view-models/tab-drag-view-model.ts';
import type { TabCardViewModel } from '../view-models/tab-card-view-model.ts';

/** What a tab's icon is told. */
export interface TabIconProps {
  /** What it shows. */
  kind: ReturnType<typeof iconKind>;
}

/** The page's favicon; one that will not decode falls back to the globe. */
function Favicon({ src }: { /** The data: URL. */ src: string }) {
  const [broken, setBroken] = useState<string | null>(null);
  if (broken === src) return <Icon name="globe" />;
  // An image is draggable by default, and the page's drag-and-drop would cancel the tab's own drag.
  return <img alt="" draggable={false} src={src} onError={() => setBroken(src)} />;
}

/** The tab's icon. */
export function TabIcon({ kind }: TabIconProps) {
  const key = typeof kind === 'string' ? kind : kind.favicon;
  return (
    <span className="tab-icon" aria-hidden="true" data-key={key}>
      {kind === 'loading' && <span className="tab-spinner"></span>}
      {kind === 'home' && (
        <svg className="tab-mark" viewBox="0 0 512 512">
          <use href="#oya-mark" />
        </svg>
      )}
      {kind === 'globe' && <Icon name="globe" />}
      {typeof kind === 'object' && <Favicon src={kind.favicon} />}
    </span>
  );
}

/** What a tab is told. */
export interface TabItemProps {
  /** The item it draws. */
  item: TabItemModel;
  /** How far it slides during a drag, in pixels (undefined at rest). */
  slide: number | undefined;
  /** It is the tab being dragged. */
  dragging: boolean;
  /** The strip. */
  strip: TabStripViewModel;
  /** The drag. */
  drag: TabDragViewModel;
  /** The hover card. */
  card: TabCardViewModel;
  /** The tab list element, which a press measures. */
  list: RefObject<HTMLElement | null>;
}

/** The class list of an item. */
function itemClass({ tab, closing, opening }: TabItemModel, dragging: boolean): string {
  const flags = {
    active: !!tab.active && !closing,
    agent: !!tab.agent,
    pinned: !!tab.pinned,
    sharing: !!tab.sharing,
    failed: !!loadError(tab),
    closing,
    opening,
    dragging,
  };
  return ['tab-item', ...Object.entries(flags).flatMap(([name, on]) => (on ? [name] : []))].join(' ');
}

/** What a screen reader adds after the title: pinned, shared in a call, or both. */
function tabDescription({ pinned, sharing }: TabItemModel['tab']): string | undefined {
  if (pinned && sharing) return TEXT.pinnedSharing;
  if (sharing) return TEXT.sharing;
  return pinned ? TEXT.pinned : undefined;
}

/** One tab. */
export function TabItem({ item, slide, dragging, strip, drag, card, list }: TabItemProps) {
  const { tab, closing } = item;
  const ctx = { tab, strip, drag, card, list };
  return (
    <div
      className={itemClass(item, dragging)}
      data-id={tab.id}
      inert={closing}
      style={slide === undefined ? undefined : { transform: `translateX(${slide}px)` }}
      {...(closing ? {} : { ...pointerHandlers(ctx), ...clickHandlers(ctx) })}
    >
      <TabIcon kind={iconKind(tab)} />
      <button
        className="tab-title"
        role={closing ? undefined : 'tab'}
        aria-selected={!!tab.active}
        tabIndex={tab.active ? 0 : -1}
        aria-description={tabDescription(tab)}
        onClick={() => strip.activate(tab.id)}
      >
        {tabTitle(tab)}
      </button>
      {!tab.pinned && <TabClose tab={tab} strip={strip} />}
    </div>
  );
}

/** A tab's close button. A pinned tab has none, so a stray click cannot lose it; Delete, middle-click and Cmd+W still close it. */
function TabClose({
  tab,
  strip,
}: Pick<TabItemProps, 'strip'> & { /** The tab it closes. */ tab: TabItemModel['tab'] }) {
  return (
    <IconButton
      className="tab-close"
      tabIndex={-1}
      aria-label={closeLabel(tab)}
      onClick={() => strip.closeByMouse(tab.id)}
      icon="close"
    />
  );
}
