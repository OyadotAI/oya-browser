/** Searchable keyboard reference, generated from the bindings used by the native shell. */
import { useState } from 'react';
import { SHORTCUTS, shortcutLabel, type Shortcut } from '../../../../shared/shortcuts.ts';
import './shortcut-guide.css';

/** The keyboard platform whose modifiers the guide should teach. */
interface Props {
  /** Node or navigator platform spelling. */
  platform: string;
}
/** A section of reference rows, including supported alternative chords. */
function ShortcutSection({
  name,
  rows,
  platform,
}: Props & { /** Section heading. */ name: string; /** Matching bindings. */ rows: readonly Shortcut[] }) {
  return (
    <section aria-label={name} className="shortcut-section">
      <h3>{name}</h3>
      <dl>
        {rows.map((row) => (
          <div key={row[0] + row[3] + row[4]}>
            <dt>{row[1]}</dt>
            <dd>
              <kbd>{shortcutLabel(row, platform)}</kbd>
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
/** Search focuses on opening; the parent dialog traps Tab and restores focus on Escape. */
export function ShortcutGuide({ platform }: Props) {
  const [query, setQuery] = useState('');
  const rows = SHORTCUTS.filter((row) =>
    `${row[1]} ${row[2]} ${shortcutLabel(row, platform)}`.toLowerCase().includes(query.trim().toLowerCase()),
  );
  const sections = [...new Set(rows.map((row) => row[2]))];
  return (
    <div className="shortcut-guide">
      <p className="shortcut-intro">Less reaching. More doing. Search a task or a key.</p>
      <input
        id="shortcut-search"
        type="search"
        aria-label="Search keyboard shortcuts"
        placeholder="Search shortcuts…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      <p className="shortcut-navigation">
        <kbd>Tab</kbd> Move between controls <span>·</span> <kbd>Esc</kbd> Close dialogs
      </p>
      <div className="shortcut-results" aria-live="polite">
        {sections.map((name) => (
          <ShortcutSection key={name} name={name} rows={rows.filter((row) => row[2] === name)} platform={platform} />
        ))}
        {!rows.length && (
          <p className="shortcut-empty">No shortcuts match “{query}”. Try “tab”, “history”, or “Ask”.</p>
        )}
      </div>
    </div>
  );
}
