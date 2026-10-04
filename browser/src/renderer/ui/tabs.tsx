/**
 * A row of tabs: the `tablist` and its `tab` buttons. Only the selected tab
 * is in the Tab order (roving focus); the arrow keys move between tabs
 * through the handler the caller passes the list (`useRovingFocus`).
 */
import type { ButtonHTMLAttributes, HTMLAttributes, Ref } from 'react';

/** What a TabList is given: a div's attributes. */
export interface TabListProps extends HTMLAttributes<HTMLDivElement> {
  /** The list element, for a caller that finds its tabs. */
  ref?: Ref<HTMLDivElement>;
}

/** The tab row. */
export function TabList(props: TabListProps) {
  return <div role="tablist" {...props} />;
}

/** What a Tab is given: a button's attributes, and whether it is the selected one. */
export interface TabProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** It is the selected tab. */
  selected: boolean;
}

/** One tab; the selected one alone takes Tab focus. */
export function Tab({ selected, ...rest }: TabProps) {
  return <button role="tab" aria-selected={selected} tabIndex={selected ? 0 : -1} {...rest} />;
}
