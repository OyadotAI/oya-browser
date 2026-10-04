/**
 * The Source pane (`#pane-source`): the format picker and Refresh, the line
 * naming the page read, and the two halves (the read, and the HTML), each
 * with Copy.
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { RendererServices } from '../../../app/services.ts';
import { FORMAT_LABELS, SOURCE_TEXT } from '../model/constants.ts';
import { sourceLook, type HalfLook, type SourceHalf, type SourceViewModel } from '../view-models/source-view-model.ts';
import { Button } from '../../../ui/index.ts';
import './inspect.css';

/** What the Source pane is given. */
export interface SourcePaneProps {
  /** The pane. */
  source: SourceViewModel;
  /** The workspace panel: the pane in view. */
  panel: RendererServices['panel'];
}

/** What one half is given. */
interface HalfProps {
  /** The pane. */
  source: SourceViewModel;
  /** Which half. */
  which: SourceHalf;
  /** How it shows. */
  look: HalfLook;
  /** Its heading. */
  label: string;
}

/** The ids of one half's parts. */
interface HalfIds {
  /** The heading. */
  label: string;
  /** The Copy button. */
  copy: string;
  /** The text. */
  content: string;
}

/** The ids of each half's parts. */
const HALF_IDS: Readonly<Record<SourceHalf, HalfIds>> = {
  page: { label: 'source-page-label', copy: 'source-copy-page', content: 'source-markdown' },
  html: { label: 'source-html-label', copy: 'source-copy-html', content: 'source-html' },
};

/** One half: its heading with Copy, then its text or placeholder. */
function Half({ source, which, look, label }: HalfProps) {
  const ids = HALF_IDS[which];
  const copyLabel = useViewModel(source).copyLabels[which];
  const cls = ['source-content', look.empty && 'empty', look.error && 'error'].filter(Boolean).join(' ');
  return (
    <section className="source-block" aria-labelledby={ids.label}>
      <div className="source-block-head">
        <h3 className="source-panel-label" id={ids.label}>
          {label}
        </h3>
        <Button id={ids.copy} type="button" disabled={look.empty} onClick={() => void source.copy(which)}>
          {copyLabel}
        </Button>
      </div>
      <div className={cls} id={ids.content} tabIndex={0}>
        {look.text}
      </div>
    </section>
  );
}

/** The Source pane, shown while the panel is on it. */
export function SourcePane({ source, panel }: SourcePaneProps) {
  const { pane } = useViewModel(panel);
  const state = useViewModel(source);
  const look = sourceLook(state);
  return (
    <div className={pane === 'source' ? 'dev-pane active' : 'dev-pane'} id="pane-source">
      <div className="source-tabs">
        <div className="source-formats" role="radiogroup" aria-label="Page format">
          {Object.entries(FORMAT_LABELS).map(([value, label]) => (
            <label key={value}>
              <input
                type="radio"
                name="source-format"
                value={value}
                checked={state.format === value}
                onChange={() => void source.pick(value)}
              />
              {label}
            </label>
          ))}
        </div>
        <Button
          className="source-refresh"
          id="source-refresh"
          disabled={state.refreshing}
          onClick={() => void source.refresh()}
        >
          {state.refreshing ? SOURCE_TEXT.reading : SOURCE_TEXT.refresh}
        </Button>
      </div>
      <p className={state.stale ? 'source-url stale' : 'source-url'} id="source-url">
        {look.url}
      </p>
      <div className="source-split">
        <Half source={source} which="page" look={look.page} label={look.label} />
        <Half source={source} which="html" look={look.html} label="HTML" />
      </div>
    </div>
  );
}
