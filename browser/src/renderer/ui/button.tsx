/**
 * The shell's buttons: a text button in one of the stylesheet's three weights
 * (text, primary, secondary) and an icon-only button. Both pass every native
 * attribute through, so a caller keeps its own id, type, title and ARIA.
 */
import type { ButtonHTMLAttributes } from 'react';
import { Icon } from './icon.tsx';
import './button.css';

/** A text button's weight: the class `<variant>-button` draws it. */
export type ButtonVariant = 'text' | 'primary' | 'secondary';

/** What a Button is given: a native button's attributes, and its weight. */
export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** How heavy it looks (text by default). */
  variant?: ButtonVariant;
}

/** Joins a base class with the caller's extra classes. */
const withClass = (base: string, extra: string | undefined): string => (extra ? `${base} ${extra}` : base);

/** A text button. */
export function Button({ variant = 'text', className, ...rest }: ButtonProps) {
  return <button className={withClass(`${variant}-button`, className)} {...rest} />;
}

/** What an IconButton is given: a native button's attributes, its icon and its name. */
export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  /** The icon it shows. */
  icon: string;
  /** Its name, as both its tooltip and its accessible name (an explicit `title` or `aria-label` wins). */
  label?: string;
  /** A toggle that is set (adds the `on` class). */
  on?: boolean;
}

/** An icon-only button; its class is `icon-button` unless the caller names another. */
export function IconButton({ icon, label, on, className = 'icon-button', ...rest }: IconButtonProps) {
  return (
    <button className={on ? `${className} on` : className} title={label} aria-label={label} {...rest}>
      <Icon name={icon} />
    </button>
  );
}
