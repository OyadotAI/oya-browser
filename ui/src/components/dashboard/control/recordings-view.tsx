/**
 * The Recordings view: a card per recorded CDP session, with Play and Delete.
 */
import { Play, Trash2 } from 'lucide-react';
import { MS_PER_SECOND, SHORT_ID_CHARS } from './constants';
import { bytes, duration, num } from './format';
import { del } from './requests';
import type { Control } from './use-control';
import type { Recording } from './types';

/** Recording cards, or how to capture one. */
export default function RecordingsView({
  ctl,
  onPlay,
}: {
  /** Control state and actions. */ ctl: Control;
  /** Opens the player on a recording. */ onPlay: (sessionId: string) => void;
}) {
  return (
    <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
      {ctl.recordings.map((r) => (
        <RecordingCard key={r.sessionId} r={r} ctl={ctl} onPlay={onPlay} />
      ))}
      {!ctl.recordings.length && (
        <p className="text-text-dim text-sm">
          No recordings. Add <code className="font-mono">?record=1</code> when connecting to capture one.
        </p>
      )}
    </div>
  );
}

/** Props for one recording card. */
interface CardProps {
  /** The recording. */
  r: Recording;
  /** Control state, for the delete action. */
  ctl: Control;
  /** Opens the player on this recording. */
  onPlay: (sessionId: string) => void;
}

/** One recording: size, length, provider, and its buttons. */
function RecordingCard({ r, ctl, onPlay }: CardProps) {
  const remove = () => void ctl.act(r.sessionId, () => del(ctl.apiKey, `/gateway/recordings/${r.sessionId}`));
  return (
    <div className="border border-border rounded-lg p-4 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-xs truncate">{r.sessionId.slice(0, SHORT_ID_CHARS)}</span>
        {r.live && <span className="text-accent text-xs">● live</span>}
      </div>
      <div className="text-xs text-text-dim space-y-0.5">
        <div>
          {num(r.frameCount)} frames · {bytes(r.bytes || 0)}
        </div>
        <div>
          {r.durationMs ? duration(r.durationMs / MS_PER_SECOND) : '—'} · {r.provider || 'unknown'}
        </div>
        {r.truncated && <div className="text-yellow">truncated at the frame cap</div>}
      </div>
      <div className="flex gap-2 pt-1">
        <button
          onClick={() => onPlay(r.sessionId)}
          disabled={!r.frameCount}
          className="flex items-center gap-1 text-xs px-2 py-1 rounded bg-bg-elevated hover:bg-border disabled:opacity-40"
        >
          <Play className="w-3 h-3" /> Play
        </button>
        <button
          onClick={remove}
          disabled={!!ctl.busy}
          className="flex items-center gap-1 text-xs px-2 py-1 rounded text-text-dim hover:text-red disabled:opacity-40"
        >
          <Trash2 className="w-3 h-3" /> Delete
        </button>
      </div>
    </div>
  );
}
