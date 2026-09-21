/**
 * The class list of a fleet-strip filter chip: highlighted while its filter is on.
 */

/** Classes for a chip; `extra` adds per-group classes such as a max width. */
export const chipClass = (active: boolean, extra = '') =>
  `inline-flex h-7 items-center gap-1.5 rounded-md border px-2 text-[12.5px] num transition-colors ${
    active
      ? 'border-accent/50 bg-accent/10 text-text'
      : 'border-border bg-transparent text-text-secondary hover:border-text/20 hover:text-text'
  } ${extra}`;
