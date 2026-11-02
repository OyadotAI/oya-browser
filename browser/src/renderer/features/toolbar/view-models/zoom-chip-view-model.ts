/**
 * The address bar's zoom chip: the shown page's zoom as main last said it, hidden
 * at actual size, and a click that puts the page back to 100%. Main owns the zoom
 * and says when a person zoomed, so the chip only shows and announces what it is told.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { PageZoom } from '../../../../shared/ipc.ts';
import { FULL_SIZE_PERCENT, ZOOM_TEXT } from '../model/constants.ts';

/** What the chip shows. */
export interface ZoomChipState {
  /** The shown page's zoom as a percentage; the chip hides at 100. */
  percent: number;
  /** What a screen reader last heard about the zoom; '' until a person zooms. */
  announcement: string;
}

/** The parts of the bridge the chip uses. */
export type ZoomChipBridge = Pick<OyaBrowser, 'onPageZoom' | 'resetZoom'>;

/** What a screen reader hears for `zoom`, or the last thing it heard when nobody zoomed. */
export function announcementFor(zoom: PageZoom, last: string): string {
  if (!zoom.announce) return last;
  return zoom.percent === FULL_SIZE_PERCENT ? ZOOM_TEXT.reset : ZOOM_TEXT.announce(zoom.percent);
}

/** The zoom chip. */
export class ZoomChipViewModel extends ViewModel<ZoomChipState> {
  /** The main process. */
  private readonly bridge: ZoomChipBridge;

  /** Hidden at 100% until main reports another zoom. */
  constructor(bridge: ZoomChipBridge) {
    super({ percent: FULL_SIZE_PERCENT, announcement: '' });
    this.bridge = bridge;
    this.own(bridge.onPageZoom((zoom) => this.received(zoom)));
  }

  /** Whether the chip shows: the page is not at actual size. */
  get shown(): boolean {
    return this.state.percent !== FULL_SIZE_PERCENT;
  }

  /** Puts the page back to actual size; a refusal (an agent took the tab) keeps the number. */
  reset(): void {
    void this.bridge.resetZoom().catch(() => {});
  }

  /** Main sent the shown page's zoom. */
  private received(zoom: PageZoom): void {
    this.set({ percent: zoom.percent, announcement: announcementFor(zoom, this.state.announcement) });
  }
}
