/**
 * The address bar (index.html's .address-wrap): the globe, the #url-bar input,
 * the loading status and the zoom chip. Enter loads what it says and lets go of the focus;
 * a focus request (Cmd/Ctrl L) focuses and selects it. The page's title
 * becomes the window's.
 */
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon, IconButton } from '../../../ui/index.ts';
import { navLook, windowTitle, type ToolbarViewModel } from '../view-models/toolbar-view-model.ts';
import { AddressSuggestions } from './address-suggestions.tsx';
import { ZoomChip } from './zoom-chip.tsx';
import { TEXT } from '../model/constants.ts';

/** What the address bar is given. */
export interface AddressBarProps {
  /** The toolbar's ViewModel. */
  vm: ToolbarViewModel;
}

/** Composition input is text, not a navigation command. */
function addressKey(event: KeyboardEvent<HTMLInputElement>, vm: ToolbarViewModel): void {
  if (event.nativeEvent.isComposing || event.currentTarget.readOnly) return;
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    vm.completion.move(event.key === 'ArrowDown' ? 1 : -1);
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    vm.escape();
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    vm.submit();
    event.currentTarget.blur();
  }
  if (event.key === 'Tab') vm.completion.dismiss();
}

/** Whether an edit added text (typing, pasting), which may complete inline; a deletion never does. */
const typed = (event: Event): boolean => String((event as InputEvent).inputType ?? '').startsWith('insert');

/** The editable combobox owns focus and keyboard interaction, not the option rows. */
function AddressInput({ vm }: AddressBarProps) {
  const { url, nav, focusRequest } = useViewModel(vm);
  const { selected, open, inline } = useViewModel(vm.completion);
  const input = useRef<HTMLInputElement>(null);
  // The completed rest stays selected, so the next key types over it and Enter accepts it.
  useEffect(() => {
    // Only with a completion: an edit in the middle of the address keeps its caret.
    if (inline) input.current?.setSelectionRange(url.length, url.length + inline.length);
  }, [url, inline]);
  useEffect(() => {
    if (focusRequest) {
      input.current?.focus();
      input.current?.select();
    }
  }, [focusRequest]);
  return (
    <input
      className="url-bar"
      id="url-bar"
      ref={input}
      aria-label={TEXT.address}
      aria-busy={nav.loading}
      role="combobox"
      aria-autocomplete="list"
      aria-expanded={open}
      aria-controls={open ? 'address-options' : undefined}
      aria-activedescendant={open && selected >= 0 ? `address-option-${selected}` : undefined}
      type="text"
      placeholder={TEXT.address}
      spellCheck={false}
      autoComplete="off"
      value={url + inline}
      onFocus={() => (vm.focused(), void vm.completion.search(vm.state.url))}
      onBlur={() => vm.blurred()}
      onChange={(event) => vm.edit(event.target.value, typed(event.nativeEvent))}
      onKeyDown={(event) => addressKey(event, vm)}
    />
  );
}

/** The star at the address bar's end, as in Chrome: filled when the page is bookmarked; a click saves or removes it. */
function BookmarkStar({ vm }: AddressBarProps) {
  // The page, not the text: the star toggles the page that is open, whatever is being typed.
  const { page } = useViewModel(vm);
  const { saved } = useViewModel(vm.bookmarks);
  if (!/^https?:\/\//i.test(page)) return null;
  const on = saved.includes(page);
  return (
    <IconButton
      className={on ? 'bookmark-star saved' : 'bookmark-star'}
      id="btn-bookmark"
      label={on ? TEXT.unbookmarkPage : TEXT.bookmarkPage}
      aria-pressed={on}
      icon="star"
      onClick={(event) => (vm.bookmarks.toggleCurrent(), event.currentTarget.blur())}
    />
  );
}

/** The address bar and its local completion dropdown. */
export function AddressBar({ vm }: AddressBarProps) {
  const { title, nav } = useViewModel(vm);
  const look = navLook(nav);
  useEffect(() => void (document.title = windowTitle(title)), [title]);
  return (
    <div className="address-wrap">
      <span className="address-icon" data-icon="globe" aria-hidden="true">
        <Icon name="globe" />
      </span>
      <AddressInput vm={vm} />
      <span
        id="navigation-status"
        role="status"
        aria-live="polite"
        title={look.statusTitle}
        aria-label={look.statusLabel}
      >
        {look.status}
      </span>
      <ZoomChip vm={vm.zoom} />
      <BookmarkStar vm={vm} />
      <AddressSuggestions vm={vm} />
    </div>
  );
}
