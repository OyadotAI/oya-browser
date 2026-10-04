/**
 * The shell page's design system: the presentational pieces every feature
 * draws with. Each renders the exact markup (classes, ids, ARIA) the
 * stylesheets and the Electron tests know. Each imports its own stylesheet;
 * the type faces come first.
 */
import './type.css';
export { Icon, iconPath, iconSvg, ICON_PATHS, type IconName, type IconProps } from './icon.tsx';
export { Orb, orbState, ORB_STATES, type OrbState, type OrbSize, type OrbProps } from './orb.tsx';
export { Button, IconButton, type ButtonProps, type ButtonVariant, type IconButtonProps } from './button.tsx';
export { Dialog, type DialogProps } from './dialog.tsx';
export { TabList, Tab, type TabListProps, type TabProps } from './tabs.tsx';
export { TextField, type TextFieldProps } from './text-field.tsx';
export { StatusLine, type StatusLineProps } from './status-line.tsx';
export { TimeAgo, type TimeAgoProps } from './time-ago.tsx';
export { ago } from './ago.ts';
