/**
 * The visible elements from the last analyze. Clicking one clicks it in the
 * browser, which needs a person to hold control.
 */
import type { PageElement, Send } from './types';

/** The elements, and what clicking them does. */
interface Props {
  /** Visible elements. */
  elements: PageElement[];
  /** Hides the list. */
  onHide: () => void;
  /** Whether a person holds control. */
  human: boolean;
  /** Why the elements are not clickable, as a tooltip. */
  needsControl?: string;
  /** Adds an optimistic activity line. */
  onInput: (line: string) => void;
  /** Sends the click. */
  send: Send;
}

/** Heading with a hide link, then one button per element. */
export default function ElementsList({ elements, onHide, human, needsControl, onInput, send }: Props) {
  const click = (id: number) => {
    onInput(`click #${id}`);
    send('click', { element_id: id, selector: `[data-ac-id="${id}"]` });
  };
  return (
    <div>
      <div className="mb-1.5 flex items-center justify-between">
        <h3 className="label mb-0">Elements ({elements.length} visible)</h3>
        <button className="text-[11.5px] text-text-muted hover:text-text" onClick={onHide}>
          hide
        </button>
      </div>
      <div className="max-h-[200px] overflow-y-auto rounded-md border border-border bg-bg font-mono text-[12px]">
        {elements.map((e) => (
          <button
            key={e.id}
            disabled={!human}
            title={needsControl}
            onClick={() => click(e.id)}
            className="flex w-full items-center gap-2 border-b border-border/60 px-2 py-1 text-left hover:bg-text/5 disabled:cursor-default disabled:hover:bg-transparent"
          >
            <span className="w-8 shrink-0 text-right text-accent">#{e.id}</span>
            <span className="w-14 shrink-0 text-text-muted">{e.type}</span>
            <span className="truncate text-text-secondary">{e.text || ''}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
