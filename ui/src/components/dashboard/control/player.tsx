/**
 * The recording player: a frame scrubber in a dialog. Frames are fetched
 * individually and cached by the browser.
 */
import Image from 'next/image';
import { Pause, Play } from 'lucide-react';
import Dialog from '@/components/ui/dialog';
import { FRAME_HEIGHT, FRAME_WIDTH, MS_PER_SECOND, PLAYER_ID_CHARS } from './constants';
import { usePlayer } from './use-player';

/** Player props. */
interface Props {
  /** Recording to play. */
  sessionId: string;
  /** Key the frames are fetched with. */
  apiKey: string;
  /** Closes the player. */
  onClose: () => void;
}

/** Plays a recording, with pause and a position slider. */
export default function Player({ sessionId, apiKey, onClose }: Props) {
  const p = usePlayer(sessionId, apiKey);
  return (
    <Dialog
      open
      onClose={onClose}
      title="Recording"
      size="lg"
      description={<span className="font-mono">{sessionId.slice(0, PLAYER_ID_CHARS)}</span>}
    >
      <div className="-mx-5 -my-4">
        <div className="bg-black flex items-center justify-center min-h-[300px]">
          {p.error ? (
            <p className="text-text-dim text-sm p-8">{p.error}</p>
          ) : p.src ? (
            <Image
              unoptimized
              src={p.src}
              alt={`Frame ${p.index + 1}`}
              width={FRAME_WIDTH}
              height={FRAME_HEIGHT}
              className="max-h-[70vh] w-auto object-contain"
            />
          ) : (
            <p className="text-text-dim text-sm p-8">Loading…</p>
          )}
        </div>
        <PlayerControls p={p} />
      </div>
    </Dialog>
  );
}

/** Play/pause, the position slider and the frame counter. */
function PlayerControls({ p }: { /** The player state. */ p: ReturnType<typeof usePlayer> }) {
  return (
    <div className="flex items-center gap-3 px-4 py-3 border-t border-border">
      <button onClick={p.toggle} className="text-text-dim hover:text-text">
        {p.playing ? <Pause className="w-4 h-4" /> : <Play className="w-4 h-4" />}
      </button>
      <input
        type="range"
        min={0}
        max={Math.max(0, p.frames.length - 1)}
        value={p.index}
        onChange={(e) => p.seek(Number(e.target.value))}
        className="flex-1 accent-accent"
        aria-label="Recording position"
      />
      <span className="font-mono text-xs text-text-dim tabular-nums whitespace-nowrap">
        {p.index + 1}/{p.frames.length} · {((p.frames[p.index]?.t ?? 0) / MS_PER_SECOND).toFixed(1)}s
      </span>
    </div>
  );
}
