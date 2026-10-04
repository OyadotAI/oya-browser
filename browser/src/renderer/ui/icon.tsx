/**
 * The shell's line icons: one path per name, drawn as an inline SVG. `Icon`
 * draws one in a view; `iconSvg` gives its markup for text that is set as HTML.
 */
import './icon.css';

/** SVG path data by icon name. */
export const ICON_PATHS = {
  back: 'm14 5-7 7 7 7M7 12h13',
  forward: 'm10 5 7 7-7 7M4 12h13',
  reload: 'M20 7v5h-5M19 12a7 7 0 1 1-2-5l3 3',
  plus: 'M12 5v14M5 12h14',
  close: 'm6 6 12 12M6 18 18 6',
  menu: 'M5 6h14M5 12h14M5 18h14',
  globe: 'M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM3 12h18M12 3c-5 5-5 13 0 18 5-5 5-13 0-18Z',
  record: 'M19 12a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z',
  stop: 'M6 6h12v12H6Z',
  panel: 'M3 4h18v16H3ZM15 4v16',
  send: 'm5 12 7-7 7 7M12 5v15',
  spark: 'm12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5Z',
  chevron: 'm9 5 7 7-7 7',
  profile: 'M16 8a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM4 21v-2a8 8 0 0 1 16 0v2',
  check: 'm5 12 4 4L19 6',
  copy: 'M9 9h11v11H9ZM5 15H4V4h11v1',
  playbook: 'M6 3h12v18l-6-4-6 4Z',
  code: 'm8 7-5 5 5 5m8-10 5 5-5 5',
  scan: 'M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5M7 8h10M7 12h10M7 16h6',
  camera: 'M3 7h4l2-3h6l2 3h4v13H3ZM16 13a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z',
  down: 'M12 4v16m-6-6 6 6 6-6',
  up: 'M12 20V4m-6 6 6-6 6 6',
  expand: 'M14 4h6v6M10 20H4v-6M20 4l-7 7M4 20l7-7',
  more: 'M5.5 12h.5M11.75 12h.5M18 12h.5',
  power: 'M12 3v8M6.4 6.4a8 8 0 1 0 11.2 0',
  attach: 'm20 11-8.5 8.5a5 5 0 0 1-7-7L13 4a3.3 3.3 0 0 1 4.7 4.7L9.2 17.2a1.7 1.7 0 0 1-2.4-2.4L14.5 7',
  trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3',
  search: 'M17 11a6 6 0 1 1-12 0 6 6 0 0 1 12 0ZM20 20l-4.35-4.35',
} as const;

/** An icon's name. */
export type IconName = keyof typeof ICON_PATHS;

/** The path for `name`; an unknown name draws the code icon. */
export const iconPath = (name: string): string =>
  Object.hasOwn(ICON_PATHS, name) ? ICON_PATHS[name as IconName] : ICON_PATHS.code;

/** The SVG attributes every icon shares. */
const SVG_ATTRS =
  'class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"';

/** An icon's SVG markup, for HTML built as text. */
export const iconSvg = (name: string): string => `<svg ${SVG_ATTRS}><path d="${iconPath(name)}"/></svg>`;

/** What `Icon` draws. */
export interface IconProps {
  /** The icon's name; an unknown name draws the code icon. */
  name: string;
}

/** Draws the icon `name`. */
export function Icon({ name }: IconProps) {
  return (
    <svg
      className="icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.65"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={iconPath(name)} />
    </svg>
  );
}
