/**
 * The address bar (index.html's .address-wrap): the globe, the #url-bar input
 * and the loading status. Enter loads what it says and lets go of the focus;
 * a focus request (Cmd/Ctrl L) focuses and selects it. The page's title
 * becomes the window's.
 */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon } from '../../../ui/index.ts';
import { navLook, windowTitle, type ToolbarViewModel } from '../view-models/toolbar-view-model.ts';
import { SUBMIT_KEY, TEXT } from '../model/constants.ts';

/** What the address bar is given. */
export interface AddressBarProps {
  /** The toolbar's ViewModel. */
  vm: ToolbarViewModel;
}

/** The address bar. */
export function AddressBar({ vm }: AddressBarProps) {
  const { url, title, nav, focusRequest } = useViewModel(vm);
  const input = useRef<HTMLInputElement>(null);
  const look = navLook(nav);
  useEffect(() => void (document.title = windowTitle(title)), [title]);
  useEffect(() => {
    if (focusRequest) input.current?.focus();
    if (focusRequest) input.current?.select();
  }, [focusRequest]);
  return (
    <div className="address-wrap">
      <span className="address-icon" data-icon="globe" aria-hidden="true">
        <Icon name="globe" />
      </span>
      <input
        className="url-bar"
        id="url-bar"
        ref={input}
        aria-label={TEXT.address}
        aria-busy={nav.loading}
        type="text"
        placeholder={TEXT.address}
        spellCheck={false}
        value={url}
        onChange={(event) => vm.edit(event.target.value)}
        onKeyDown={(event) => event.key === SUBMIT_KEY && (vm.submit(), event.currentTarget.blur())}
      />
      <span
        id="navigation-status"
        role="status"
        aria-live="polite"
        title={look.statusTitle}
        aria-label={look.statusLabel}
      >
        {look.status}
      </span>
    </div>
  );
}
