/**
 * The navigation toolbar (index.html's nav.toolbar): Back, Forward, Reload,
 * the address bar and its status, then the other features' toolbar parts
 * (update pill, connection pill, control status) as children, the Ask button
 * and the navigation progress line. Its ids stay: the control guard finds
 * #btn-back, #btn-forward, #btn-reload and #url-bar by id and marks them
 * blocked or read-only while an agent drives.
 */
import type { ReactNode } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { IconButton } from '../../../ui/index.ts';
import './toolbar.css';
import type { RendererServices } from '../../../app/services.ts';
import { navLook, type ToolbarViewModel } from '../view-models/toolbar-view-model.ts';
import { AddressBar } from './address-bar.tsx';
import { AskButton } from './ask-button.tsx';
import { TEXT } from '../model/constants.ts';

/** What the toolbar is given. */
export interface ToolbarProps extends Pick<RendererServices, 'panel' | 'shell'> {
  /** The toolbar's ViewModel. */
  vm: ToolbarViewModel;
  /** The other features' toolbar parts, between the address bar and the Ask button. */
  children?: ReactNode;
}

/** A text button keeps the library discoverable without introducing an ambiguous icon. */
function LibraryButton({ vm }: Pick<ToolbarProps, 'vm'>) {
  return (
    <button
      className="library-button"
      title={TEXT.libraryTitle}
      aria-label={TEXT.libraryTitle}
      onClick={() => vm.showLibrary()}
    >
      {TEXT.library}
    </button>
  );
}

/** The navigation toolbar. */
export function Toolbar({ vm, panel, shell, children }: ToolbarProps) {
  const { nav } = useViewModel(vm);
  const look = navLook(nav);
  return (
    <nav className="toolbar" aria-label={TEXT.toolbar}>
      <IconButton
        className="nav-btn"
        id="btn-back"
        label={TEXT.back}
        data-icon="back"
        disabled={!nav.canGoBack}
        onClick={() => vm.back()}
        icon="back"
      />
      <IconButton
        className="nav-btn"
        id="btn-forward"
        label={TEXT.forward}
        data-icon="forward"
        disabled={!nav.canGoForward}
        onClick={() => vm.forward()}
        icon="forward"
      />
      <IconButton
        className="nav-btn"
        id="btn-reload"
        label={look.reloadLabel}
        data-icon={look.reloadIcon}
        onClick={() => vm.reload()}
        icon={look.reloadIcon}
      />
      <AddressBar vm={vm} />
      <LibraryButton vm={vm} />
      {children}
      <AskButton panel={panel} shell={shell} />
      <div id="navigation-progress" hidden={!nav.loading} aria-hidden="true"></div>
    </nav>
  );
}
