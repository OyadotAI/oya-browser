/**
 * The small pieces of window chrome: the brand mark in the tab bar
 * (#brand-orb, turning while the agent works), the appearance picker for the
 * shell dialog's footer (#theme-preference), and the page backdrop
 * (#page-backdrop, a still of the page behind a dialog).
 */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import type { AgentActivityViewModel } from '../view-models/activity-view-model.ts';
import { themeOf, type ThemeViewModel } from '../view-models/theme-view-model.ts';
import type { BackdropViewModel } from '../view-models/backdrop-view-model.ts';
import { TEXT, THEMES } from '../model/constants.ts';
import './chrome-views.css';

/** What a chrome piece is given: its ViewModel. */
export interface ChromeProps<VM> {
  /** The ViewModel it reads. */
  vm: VM;
}

/** The window's brand mark, first in the tab bar. */
export function BrandMark({ vm }: ChromeProps<AgentActivityViewModel>) {
  const { active } = useViewModel(vm);
  return (
    <svg
      className="oya-mark window-brand"
      id="brand-orb"
      viewBox="0 0 512 512"
      aria-hidden="true"
      data-state={active ? 'thinking' : 'idle'}
    >
      <circle className="mark-back" cx="276" cy="276" r="160" />
      <circle className="mark-front" cx="236" cy="236" r="160" />
    </svg>
  );
}

/** The appearance picker: its label and select, for the shell dialog's footer. */
export function ThemeSelect({ vm }: ChromeProps<ThemeViewModel>) {
  const { theme } = useViewModel(vm);
  return (
    <>
      <label htmlFor="theme-preference">{TEXT.appearance}</label>
      <select id="theme-preference" value={theme} onChange={(event) => vm.choose(themeOf(event.target.value))}>
        {THEMES.map((name) => (
          <option key={name} value={name}>
            {TEXT.themes[name]}
          </option>
        ))}
      </select>
    </>
  );
}

/** The page still: placed over the page, shown once decoded. */
export function PageBackdrop({ vm }: ChromeProps<BackdropViewModel>) {
  const { still, shown } = useViewModel(vm);
  const image = useRef<HTMLImageElement>(null);
  useEffect(() => {
    if (!still || !image.current) return;
    void image.current
      .decode()
      .catch(() => {})
      .then(() => vm.decoded(still.token));
  }, [still, vm]);
  const bounds = still?.bounds;
  const place = bounds && { left: bounds.x, top: bounds.y, width: bounds.width, height: bounds.height };
  return <img id="page-backdrop" alt="" hidden={!shown} src={still?.image} style={place} ref={image} />;
}
