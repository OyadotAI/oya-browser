/**
 * The Ask button (index.html's #btn-dev): opens the workspace panel where it
 * was left (on Record while recording), or closes it. It shows the panel's
 * state (active, aria-expanded) and a recording dot while a workflow is
 * recorded.
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { RendererServices } from '../../../app/services.ts';
import { TEXT } from '../model/constants.ts';
import { askClass } from '../view-models/toolbar-view-model.ts';

/** What the Ask button is given. */
export type AskButtonProps = Pick<RendererServices, 'panel' | 'shell'>;

/** The Ask button. */
export function AskButton({ panel, shell }: AskButtonProps) {
  const { open } = useViewModel(panel);
  const { recording } = useViewModel(shell);
  return (
    <button
      className={askClass(open, recording)}
      id="btn-dev"
      aria-expanded={open}
      title={recording ? TEXT.askRecordingTitle : TEXT.askTitle}
      onClick={() => void panel.toggle(recording)}
    >
      <span className="agent-dot" aria-hidden="true"></span>
      <svg className="oya-mark agent-mark" viewBox="0 0 512 512" aria-hidden="true">
        <circle className="mark-back" cx="276" cy="276" r="160" />
        <circle className="mark-front" cx="236" cy="236" r="160" />
      </svg>
      <span className="button-label">{TEXT.ask}</span>
    </button>
  );
}
