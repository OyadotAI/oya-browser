/**
 * The settings' default page format (index.html's #page-format-preference):
 * how the agent and the Source pane read pages. It shows and sets the Source
 * pane's format, which saves the choice.
 */
import { useViewModel } from '../hooks/index.ts';
import { FORMAT_LABELS, type SourceViewModel } from '../features/inspect/index.ts';

/** What the select is given. */
export interface PageFormatSelectProps {
  /** The Source pane, which holds the format. */
  source: SourceViewModel;
}

/** The label and the select. */
export function PageFormatSelect({ source }: PageFormatSelectProps) {
  const { format } = useViewModel(source);
  return (
    <>
      <label htmlFor="page-format-preference">Page format</label>
      <select id="page-format-preference" value={format} onChange={(e) => void source.chooseDefault(e.target.value)}>
        {Object.entries(FORMAT_LABELS).map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </select>
    </>
  );
}
