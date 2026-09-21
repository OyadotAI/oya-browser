/**
 * The panel's action row: screenshot, elements, stream, and the share links.
 */
import { Camera, ExternalLink, ScanSearch, Share2 } from 'lucide-react';
import Kbd from '@/components/ui/kbd';
import type { PanelContext } from './context';
import { openStream, share } from './share';
import type { Busy } from './types';

/** What the buttons run, and what disables them. */
interface Props {
  /** The panel's shared context. */
  ctx: PanelContext;
  /** Which action is running. */
  busy: Busy;
  /** Whether a person holds control. */
  human: boolean;
  /** Why human-only buttons are off, as a tooltip. */
  needsControl?: string;
  /** Takes a screenshot. */
  screenshot: () => void;
  /** Lists the page's elements. */
  analyze: () => void;
}

/** One row of ghost buttons. */
export default function PanelActions({ ctx, busy, human, needsControl, screenshot, analyze }: Props) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <button className="btn-ghost" onClick={screenshot} disabled={busy === 'screenshot'}>
        <Camera className="h-3.5 w-3.5" /> Screenshot <Kbd>S</Kbd>
      </button>
      {/* Human-only: re-analyzing renumbers element ids under a running agent. */}
      <button className="btn-ghost" onClick={analyze} disabled={busy === 'analyze' || !human} title={needsControl}>
        <ScanSearch className="h-3.5 w-3.5" /> Elements
      </button>
      <button
        className="btn-ghost"
        onClick={() => openStream(ctx)}
        disabled={busy === 'stream'}
        title="Open live browser in a new tab"
      >
        <ExternalLink className="h-3.5 w-3.5" /> Stream
      </button>
      <button
        className="btn-ghost"
        onClick={() => share(ctx, false)}
        disabled={busy === 'share-view'}
        title="Copy a view-only link to send to someone"
      >
        <Share2 className="h-3.5 w-3.5" /> Share view
      </button>
      <button
        className="btn-ghost"
        onClick={() => share(ctx, true)}
        disabled={busy === 'share-control'}
        title="Copy a link that lets the recipient take control"
      >
        <Share2 className="h-3.5 w-3.5" /> Share control
      </button>
    </div>
  );
}
