/**
 * The tab strip's entry: the tab bar and hover card the composition root
 * mounts, and the ViewModels it builds (the card, then the drag with the
 * card, then the strip with both).
 */
export { TabBar, type TabBarProps } from './components/tab-bar.tsx';
export { TabCard, type TabCardProps } from './components/tab-card.tsx';
export {
  TabStripViewModel,
  stripLayout,
  type TabStripDeps,
  type TabStripState,
} from './view-models/tab-strip-view-model.ts';
export { TabDragViewModel, type TabDragDeps, type TabDragState } from './view-models/tab-drag-view-model.ts';
export { TabCardViewModel, type TabCardState } from './view-models/tab-card-view-model.ts';
export type { Tab } from './model/tab-model.ts';
