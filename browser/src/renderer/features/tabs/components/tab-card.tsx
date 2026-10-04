/**
 * The tab hover card (index.html's #tab-card): the tab's full title and
 * address, under the tab, inside the window. It fits in the toolbar's height,
 * since the page's own view is drawn over the shell below it.
 */
import { useLayoutEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { cardAddress, tabTitle } from '../model/tab-model.ts';
import { cardLeft } from '../model/tab-math.ts';
import type { TabCardViewModel } from '../view-models/tab-card-view-model.ts';

/** What the card is given. */
export interface TabCardProps {
  /** The card's ViewModel. */
  vm: TabCardViewModel;
}

/** The hover card. */
export function TabCard({ vm }: TabCardProps) {
  const { tab, left } = useViewModel(vm);
  const card = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = card.current;
    if (el && tab) el.style.left = `${cardLeft(left, innerWidth, el.offsetWidth)}px`;
  }, [tab, left]);
  return (
    <div className="tab-card" id="tab-card" role="tooltip" hidden={!tab} ref={card}>
      <strong>{tab ? tabTitle(tab) : ''}</strong>
      <span>{tab ? cardAddress(tab) : ''}</span>
    </div>
  );
}
