/**
 * The address bar (index.html's .address-wrap): the globe, the #url-bar input
 * and the loading status. Enter loads what it says and lets go of the focus;
 * a focus request (Cmd/Ctrl L) focuses and selects it. The page's title
 * becomes the window's.
 */
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon } from '../../../ui/index.ts';
import { navLook, windowTitle, type ToolbarViewModel } from '../view-models/toolbar-view-model.ts';
import { AddressSuggestions } from './address-suggestions.tsx';
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
    vm.completion.dismiss();
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    vm.submit();
    event.currentTarget.blur();
  }
  if (event.key === 'Tab') vm.completion.dismiss();
}

/** The editable combobox owns focus and keyboard interaction, not the option rows. */
function AddressInput({ vm }: AddressBarProps) {
  const { url, nav, focusRequest } = useViewModel(vm);
  const { selected, open } = useViewModel(vm.completion);
  const input = useRef<HTMLInputElement>(null);
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
      value={url}
      onFocus={() => void vm.completion.search(vm.state.url)}
      onBlur={() => vm.completion.dismiss()}
      onChange={(event) => vm.edit(event.target.value)}
      onKeyDown={(event) => addressKey(event, vm)}
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
      <AddressSuggestions vm={vm} />
    </div>
  );
}
