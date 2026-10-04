/**
 * The tab bar (index.html's #tab-bar): the window's brand mark, the strip of
 * tabs, the new-tab button, the free strip that always drags the window, and
 * the commands button (another feature's, passed in). Leaving the bar lets
 * held tab widths go; leaving the strip hides the hover card.
 */
import { useRef, type CSSProperties, type ReactNode } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { IconButton } from '../../../ui/index.ts';
import './tabs.css';
import { stripLayout, type TabStripViewModel } from '../view-models/tab-strip-view-model.ts';
import { slideFor, type TabDragViewModel } from '../view-models/tab-drag-view-model.ts';
import type { TabCardViewModel } from '../view-models/tab-card-view-model.ts';
import { TabItem } from './tab-item.tsx';
import { stripKeydown } from './tab-gestures.ts';
import { useDragCancel, useDropSettle, useRevealActive, useTabRoom } from '../hooks/use-tab-strip.ts';
import { TEXT } from '../model/constants.ts';

/** What the tab bar is given. */
export interface TabBarProps {
  /** The strip. */
  strip: TabStripViewModel;
  /** The drag. */
  drag: TabDragViewModel;
  /** The hover card. */
  card: TabCardViewModel;
  /** The window's brand mark (#brand-orb), first in the bar. */
  brand: ReactNode;
  /** The commands button (#btn-commands), last in the bar; the tabs' room ends at it. */
  commands: ReactNode;
}

/** The new-tab button. */
function NewTabButton({ strip }: Pick<TabBarProps, 'strip'>) {
  return (
    <IconButton
      className="tab-new"
      id="btn-new-tab"
      aria-label={TEXT.newTab}
      title={TEXT.newTabTitle}
      data-icon="plus"
      onClick={() => strip.newTab()}
      icon="plus"
    />
  );
}

/** The tab bar. */
export function TabBar({ strip, drag, card, brand, commands }: TabBarProps) {
  const state = useViewModel(strip);
  const dragging = useViewModel(drag);
  const bar = useRef<HTMLElement>(null);
  const list = useRef<HTMLDivElement>(null);
  useTabRoom(bar, list, strip);
  useRevealActive(list, state.items);
  useDropSettle(list, state.drop);
  useDragCancel(drag);
  const layout = stripLayout(state);
  const open = state.items.filter((item) => !item.closing);
  const listClass = ['tab-list', layout.overflowing && 'overflowing', dragging.lifted && 'dragging'];
  return (
    <nav className="tab-bar" id="tab-bar" aria-label={TEXT.bar} ref={bar} onPointerLeave={() => strip.release()}>
      {brand}
      <div
        className={listClass.filter(Boolean).join(' ')}
        id="tab-list"
        role="tablist"
        aria-label={TEXT.list}
        ref={list}
        data-size={layout.size}
        style={{ '--tab-width': `${layout.width}px` } as CSSProperties}
        onKeyDown={(event) => stripKeydown(event, strip)}
        onPointerLeave={() => card.hide()}
        // Nothing on the strip starts the page's drag-and-drop: it would cancel a tab's own drag.
        onDragStart={(event) => event.preventDefault()}
      >
        {state.items.map((item) => (
          <TabItem
            key={item.tab.id}
            item={item}
            slide={item.closing ? undefined : slideFor(dragging, item.tab.id, open.indexOf(item))}
            dragging={dragging.lifted && dragging.id === item.tab.id}
            {...{ strip, drag, card, list }}
          />
        ))}
      </div>
      <NewTabButton strip={strip} />
      {/* Free strip that always drags the window, however many tabs are open. */}
      <div className="tab-drag-space" aria-hidden="true"></div>
      {commands}
    </nav>
  );
}
