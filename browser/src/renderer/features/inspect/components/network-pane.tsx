/**
 * The Activity pane (`#pane-network`): the filters, the log (`#net-log`, a
 * `.dev-entry` per message, opened by a click that did not end a text
 * selection, following the newest while scrolled to the bottom), and what
 * shows while it is empty (`#net-empty`).
 */
import { useViewModel, useStickToBottom } from '../../../hooks/index.ts';
import type { RendererServices } from '../../../app/services.ts';
import { NET_FILTER_LABELS } from '../model/constants.ts';
import {
  directionLabel,
  fmtTime,
  shows,
  type LogEntry,
  type NetLogViewModel,
} from '../view-models/net-log-view-model.ts';
import './inspect.css';

/** What the Activity pane is given. */
export interface NetworkPaneProps {
  /** The log. */
  log: NetLogViewModel;
  /** The workspace panel: the pane in view. */
  panel: RendererServices['panel'];
}

/** What a row is given. */
interface EntryProps {
  /** The log. */
  log: NetLogViewModel;
  /** The message. */
  entry: LogEntry;
  /** The row is open. */
  expanded: boolean;
  /** The active filter shows it. */
  shown: boolean;
}

/** One message: time, direction, type, and its body when open. */
function Entry({ log, entry, expanded, shown }: EntryProps) {
  const toggle = () => log.toggleRow(entry.key, String(window.getSelection?.() ?? ''));
  return (
    <div
      className={expanded ? 'dev-entry expanded' : 'dev-entry'}
      data-dir={entry.dir}
      data-type={entry.type}
      style={shown ? undefined : { display: 'none' }}
      onClick={toggle}
    >
      <div className="head">
        <span className="ts">{fmtTime(entry.ts)}</span>
        <span className={`dir ${entry.dir === 'in' ? 'in' : 'out'}`}>{directionLabel(entry.dir)}</span>
        <span className="msg-type">{entry.type}</span>
      </div>
      <div className="body">{entry.data}</div>
    </div>
  );
}

/** The Activity pane, shown while the panel is on it. */
export function NetworkPane({ log, panel }: NetworkPaneProps) {
  const { pane } = useViewModel(panel);
  const { entries, filter, expanded } = useViewModel(log);
  const { box, onScroll } = useStickToBottom<HTMLDivElement>(entries);
  return (
    <div className={pane === 'network' ? 'dev-pane active' : 'dev-pane'} id="pane-network">
      <div className="net-filters">
        {NET_FILTER_LABELS.map(([name, label]) => (
          <button
            key={name}
            className={filter === name ? 'net-filter active' : 'net-filter'}
            data-filter={name}
            onClick={() => log.setFilter(name)}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="net-log" id="net-log" ref={box} onScroll={onScroll}>
        {entries.map((entry) => (
          <Entry
            key={entry.key}
            log={log}
            entry={entry}
            expanded={expanded.includes(entry.key)}
            shown={shows(filter, entry)}
          />
        ))}
      </div>
      <p className="net-empty" id="net-empty" hidden={entries.length > 0}>
        Messages between this browser and Oya show up here.
      </p>
    </div>
  );
}
