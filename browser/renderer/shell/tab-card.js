/**
 * The tab hover card, like Chrome's: rest the pointer on a tab and its full
 * title and address show under it, instead of the native tooltip. Once one
 * card is up, moving along the strip shows the next at once. The card fits
 * in the toolbar's height, since the page's own view is drawn over the shell
 * below it.
 */
/* global Dom, RendererConstants */
/* exported TabCard */

/** The hover card. */
const TabCard = {
  /** The timer that shows the card after the pointer rests. */
  timer: 0,

  /** The pointer came onto `item`: show its card, now if one is up, else after a rest. */
  hover(item) {
    clearTimeout(TabCard.timer);
    if (!Dom.byId('tab-card').hidden) return TabCard.show(item);
    TabCard.timer = setTimeout(() => TabCard.show(item), RendererConstants.TAB_CARD_DELAY_MS);
  },

  /** Fills the card with `item`'s tab and places it under the tab, inside the window. */
  show(item) {
    const tab = item.tabData;
    if (!tab || !item.isConnected) return;
    const card = Dom.byId('tab-card');
    card.querySelector('strong').textContent = tab.title || 'New tab';
    card.querySelector('span').textContent = TabCard.address(tab);
    card.hidden = false;
    const left = item.getBoundingClientRect().left;
    card.style.left = Math.max(0, Math.min(left, innerWidth - card.offsetWidth)) + 'px';
  },

  /** What the card says the tab is showing: its address without the scheme, or where it is. */
  address(tab) {
    if (tab.home) return 'Oya start page';
    return String(tab.url || '').replace(/^https?:\/\//, '') || 'Blank page';
  },

  /** Hides the card and forgets any card waiting to show. */
  hide() {
    clearTimeout(TabCard.timer);
    Dom.byId('tab-card').hidden = true;
  },
};
