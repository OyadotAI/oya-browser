/**
 * The page backdrop: while a dialog detaches the page view, a still of the
 * page stands in for it so the window does not flash empty. The view decodes
 * the still and reports it; once it is shown and a frame has painted it, the
 * main process is told. A slower, older still never shows over a newer one.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { Bounds, Payload } from '../../../../shared/ipc.ts';

/** A still of the page, where to put it, and the request it answers. */
export interface Still {
  /** The image, as a data: URL. */
  image: string;
  /** Where the page sits in the window. */
  bounds: Bounds;
  /** The request's token, sent back once painted. */
  token: string;
}

/** What the backdrop shows. */
export interface BackdropState {
  /** The latest still, or null for none. */
  still: Still | null;
  /** The still is decoded and on screen. */
  shown: boolean;
}

/** What the backdrop uses. */
export interface BackdropDeps extends Pick<RendererServices, 'frames'> {
  /** The stills, and where to say one is painted. */
  bridge: Pick<OyaBrowser, 'onPageBackdrop' | 'backdropReady'>;
}

/** `value` as a still, or null when it is not one. */
export function stillOf(value: Payload | null): Still | null {
  const bounds = value?.bounds as Bounds | undefined;
  if (typeof value?.image !== 'string' || typeof bounds?.x !== 'number') return null;
  return { image: value.image, bounds, token: String(value.token) };
}

/** The page still. */
export class BackdropViewModel extends ViewModel<BackdropState> {
  /** What it uses. */
  private readonly deps: BackdropDeps;

  /** Hidden, following the main process's stills. */
  constructor(deps: BackdropDeps) {
    super({ still: null, shown: false });
    this.deps = deps;
    this.own(deps.bridge.onPageBackdrop((value) => this.receive(stillOf(value))));
  }

  /** The view decoded the still for `token` (or failed to): the latest shows, and is reported once painted. */
  decoded(token: string): void {
    if (this.state.still?.token !== token) return;
    this.set({ shown: true });
    this.deps.frames.request(() => void this.deps.bridge.backdropReady(token));
  }

  /** A new still (it loads behind the one shown), or none: the backdrop hides. */
  private receive(still: Still | null): void {
    this.set(still ? { still } : { still: null, shown: false });
  }
}
