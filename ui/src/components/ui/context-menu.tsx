/**
 * The right-click menu used across the console. Its behavior lives in
 * use-context-menu.ts; this file draws it.
 */
'use client';

import type { ReactNode } from 'react';
import Kbd from './kbd';
import { useContextMenu, type Point } from './use-context-menu';

/** One menu entry. */
export interface MenuItem {
  /** The entry's text. */
  label: string;
  /** Icon drawn before the label. */
  icon?: ReactNode;
  /** Key hint drawn after the label. */
  shortcut?: string;
  /** Drawn in red: the action destroys something. */
  danger?: boolean;
  /** Shown but not selectable. */
  disabled?: boolean;
  /** A thin rule above this item. */
  separator?: boolean;
  /** Runs when the item is picked. */
  onSelect: () => void;
}

/** ContextMenu's props. */
interface Props {
  /** Where it opened, or null while closed. */
  at: Point | null;
  /** The entries. */
  items: MenuItem[];
  /** Closes it. */
  onClose: () => void;
  /** Read out by assistive tech; also the menu's aria-label. */
  label: string;
}

/** One entry's props. */
interface ItemProps {
  /** The entry. */
  it: MenuItem;
  /** Whether it is the active one. */
  active: boolean;
  /** Makes it the active one. */
  onHover: () => void;
  /** Closes the menu after a pick. */
  onClose: () => void;
}

/** The entry's text color, then its highlight when active. */
function itemClass(it: MenuItem, active: boolean): string {
  const tone = it.disabled ? 'text-text-dim' : it.danger ? 'text-red' : 'text-text';
  const highlight = active && !it.disabled ? (it.danger ? 'bg-red/10' : 'bg-text/[0.06]') : '';
  return `flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-[13px] ${tone} ${highlight}`;
}

/** One entry, with its optional separator. */
function Item({ it, active, onHover, onClose }: ItemProps) {
  return (
    <div>
      {it.separator && <div className="my-1 h-px bg-border" />}
      <button
        role="menuitem"
        disabled={it.disabled}
        onMouseEnter={() => !it.disabled && onHover()}
        onClick={() => {
          if (it.disabled) return;
          it.onSelect();
          onClose();
        }}
        className={itemClass(it, active)}
      >
        {it.icon && <span className="w-4 text-text-muted [&>svg]:h-3.5 [&>svg]:w-3.5">{it.icon}</span>}
        <span className="flex-1">{it.label}</span>
        {it.shortcut && <Kbd>{it.shortcut}</Kbd>}
      </button>
    </div>
  );
}

/**
 * A right-click menu. Opens at the cursor, stays on screen, arrows move,
 * Enter picks, Escape and any click outside close it.
 */
export default function ContextMenu({ at, items, onClose, label }: Props) {
  const { ref, pos, active, setActive } = useContextMenu(at, items, onClose);
  if (!at) return null;
  return (
    <div
      ref={ref}
      role="menu"
      aria-label={label}
      tabIndex={-1}
      style={{ left: pos?.x ?? at.x, top: pos?.y ?? at.y }}
      className="fixed z-[60] min-w-[220px] rounded-lg border border-border bg-bg-card py-1 shadow-[var(--shadow-elevated)] outline-none"
    >
      {items.map((it, i) => (
        <Item key={it.label + i} it={it} active={active === i} onHover={() => setActive(i)} onClose={onClose} />
      ))}
    </div>
  );
}
