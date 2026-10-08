/** Accessible completion rows: focus stays in the combobox while arrows select a destination. */
import { useViewModel } from '../../../hooks/index.ts';
import type { AddressBarProps } from './address-bar.tsx';
import './address-suggestions.css';

/** Local history and bookmarks, never a remote provider's search suggestions. */
export function AddressSuggestions({ vm }: AddressBarProps) {
  const { items, selected, open } = useViewModel(vm.completion);
  if (!open) return null;
  return (
    <div className="address-suggestions">
      <div className="address-suggestions-heading">
        From your browser <span>↑ ↓ to select · Enter to open · Esc to close</span>
      </div>
      <div id="address-options" role="listbox" aria-label="Address suggestions">
        {items.map((item, index) => (
          <div
            key={item.url}
            id={`address-option-${index}`}
            role="option"
            aria-selected={selected === index}
            className="address-option"
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => vm.submit(item.url)}
          >
            <span className="address-option-source">{item.source === 'bookmark' ? 'Bookmark' : 'History'}</span>
            <span className="address-option-copy">
              <strong>{item.title}</strong>
              <span>{item.url}</span>
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
