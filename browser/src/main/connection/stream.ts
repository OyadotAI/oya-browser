/** The live view: frames of the active tab over the control socket while someone watches. */
import type { NativeImage, Rectangle, WebContents } from 'electron';
import WebSocket from 'ws';
import type { AppServices } from '../app/services.ts';
import type { SocketLike } from './socket.ts';
import { STREAM_MIN_FRAME_MS, MS_PER_SECOND, STREAM_MAX_BUFFERED, STREAM_JPEG_QUALITY } from './constants.ts';

/** The tab to show, and the socket the frames go over. */
type Deps = Pick<AppServices, 'tabs' | 'socket'>;

/** The view a frame is taken of: its size, and its page to capture. */
export interface StreamView {
  /** The view's size in CSS pixels. */
  getBounds(): Pick<Rectangle, 'width' | 'height'>;
  /** Its page. */
  webContents: Pick<WebContents, 'capturePage'>;
}

/**
 * capturePage() returns device pixels, 2x the page on a retina screen. The
 * live view maps a click through the frame's own width and sends it as CSS
 * pixels, so an unscaled frame puts every click at twice the distance from
 * the top-left: near enough at the corner, nowhere near the target at the
 * other edge. Send the page at the size the page thinks it is.
 */
function frameAtPageSize(view: StreamView, img: NativeImage): NativeImage {
  const { width, height } = view.getBounds();
  return width > 0 && img.getSize().width !== width ? img.resize({ width, height, quality: 'good' }) : img;
}

/** Streams the active tab while the server asks for it. */
export class LiveStream {
  /** The tabs and the control socket. */
  private readonly deps: Deps;
  /** The frame loop, while one runs. */
  private interval: ReturnType<typeof setInterval> | null = null;
  /** True while a frame is being captured, so frames never overlap. */
  private capturing = false;

  /** `deps` gives the active tab and the control socket. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Starts a frame loop at `fps`, replacing any running one. */
  startStream(fps: number): void {
    this.stopStream();
    const ms = Math.max(STREAM_MIN_FRAME_MS, Math.round(MS_PER_SECOND / fps));
    this.interval = setInterval(() => this.tick(), ms);
  }

  /** Stops the frame loop, if one runs. */
  stopStream(): void {
    if (this.interval) {
      clearInterval(this.interval);
      this.interval = null;
    }
  }

  /** Whether a frame should be taken now: someone is connected, the socket keeps up, none is in flight. */
  private canCapture(view: StreamView | null, ws: SocketLike | null): boolean {
    return !!(
      ws &&
      ws.readyState === WebSocket.OPEN &&
      view &&
      !this.capturing &&
      ws.bufferedAmount <= STREAM_MAX_BUFFERED
    );
  }

  /** Captures and sends one frame, if the socket can take it. */
  private async tick(): Promise<void> {
    const view = this.deps.tabs.getActiveView() as StreamView | null; // a tab shown in the window is a BrowserView; popups have windows of their own
    if (!view || !this.canCapture(view, this.deps.socket.ws)) return;
    this.capturing = true;
    await this.sendFrame(view).catch(() => {});
    this.capturing = false;
  }

  /** Captures the view and sends it as a JPEG frame. */
  private async sendFrame(view: StreamView): Promise<void> {
    const frame = frameAtPageSize(view, await view.webContents.capturePage());
    const data = 'data:image/jpeg;base64,' + frame.toJPEG(STREAM_JPEG_QUALITY).toString('base64');
    this.deps.socket.send({ type: 'frame', data });
  }
}
