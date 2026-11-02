/**
 * A tab carried over this strip from another window, drawn lifted in the gap
 * where it would land while its own window is hidden, as Firefox and Chrome
 * draw it. It is only a picture: the tab itself arrives when the pointer lets go.
 */
import type { IncomingTab } from '../../../../shared/ipc.ts';
import { TEXT } from '../model/constants.ts';
import { TabIcon, type TabIconProps } from './tab-item.tsx';

/** What the ghost is given. */
export interface IncomingGhostProps {
  /** The tab being carried in. */
  tab: IncomingTab;
  /** Its left edge in the strip's pixels. */
  left: number;
}

/** The icon the carried tab shows: the Oya mark on the start page, its favicon, else the globe. */
function kindOf(tab: IncomingTab): TabIconProps['kind'] {
  if (tab.home) return 'home';
  return tab.favicon ? { favicon: tab.favicon } : 'globe';
}

/**
 * The incoming tab under the pointer, and the room it adds at the strip's end: the
 * strip is as wide as its tabs, and clipped a tab drawn past the last one to a sliver.
 */
export function IncomingGhost({ tab, left }: IncomingGhostProps) {
  return (
    <>
      <div className="tab-incoming-room" aria-hidden="true"></div>
      <div className="tab-item incoming" aria-hidden="true" style={{ left }}>
        <TabIcon kind={kindOf(tab)} />
        <span className="tab-title">{tab.title || TEXT.untitled}</span>
      </div>
    </>
  );
}
