/**
 * The navigation toolbar (index.html's nav.toolbar): Back, Forward, Reload,
 * the address bar and its status, then the other features' toolbar parts
 * (update pill, control status) as children, the Ask button (the Incognito badge in
 * an incognito window, where no agent works), and at its end
 * notifications and the profile, as in Chrome; and the navigation progress line.
 * History and bookmarks are on the bookmarks bar under it (All bookmarks). Its ids stay: the control guard finds
 * #btn-back, #btn-forward, #btn-reload and #url-bar by id and marks them
 * blocked or read-only while an agent drives.
 */
import type { ReactNode } from 'react';
import { incognito, useViewModel } from '../../../hooks/index.ts';
import { IconButton } from '../../../ui/index.ts';
import './toolbar.css';
import type { RendererServices } from '../../../app/services.ts';
import { navLook, type ToolbarViewModel } from '../view-models/toolbar-view-model.ts';
import { AddressBar } from './address-bar.tsx';
import { AskButton } from './ask-button.tsx';
import { IncognitoBadge } from './incognito-badge.tsx';
import { LibraryButtons } from './library-buttons.tsx';
import { TEXT } from '../model/constants.ts';

/** What the toolbar is given. */
export interface ToolbarProps extends Pick<RendererServices, 'panel' | 'shell'> {
  /** The toolbar's ViewModel. */
  vm: ToolbarViewModel;
  /** The other features' toolbar parts, between the address bar and the Ask button. */
  children?: ReactNode;
  /** What sits after the Ask button, at the toolbar's end: notifications and the profile, as in Chrome. */
  trailing?: ReactNode;
}

/** The navigation toolbar. */
export function Toolbar({ vm, panel, shell, children, trailing }: ToolbarProps) {
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
      <ToolbarLibrary vm={vm} />
      {children}
      {incognito() ? <IncognitoBadge /> : <AskButton panel={panel} shell={shell} />}
      {trailing}
      <div id="navigation-progress" hidden={!nav.loading} aria-hidden="true"></div>
    </nav>
  );
}

/** When the bookmarks row is hidden, keep its collection buttons accessible beside the address. */
function ToolbarLibrary({ vm }: Pick<ToolbarProps, 'vm'>) {
  const { shown } = useViewModel(vm.bookmarks);
  return shown ? null : <LibraryButtons vm={vm.bookmarks} />;
}
