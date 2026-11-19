/** Native buttons that show which tools each choice works with, so people pick by what they already have. */
import { Icon } from '../../../ui/index.ts';
import { BrandChips, type Brand } from './brand-marks.tsx';
import type { SetupPath } from '../view-models/onboarding-view-model.ts';

/** Choice content stays short; the tool logos do the recognising. */
interface Props {
  /** Requested path label or recognizable provider name. */
  title: string;
  /** One short line, usually what the person needs to have. */
  caption: string;
  /** The tools this choice works with, shown as logos and names. */
  brands: Brand[];
  /** Existing navigation handler. */
  onClick: () => void;
  /** Retains Back focus restoration for the initial paths. */
  path?: SetupPath;
  /** Compact rows keep technical actions in view without privileging one route. */
  compact?: boolean;
  /** A pending login cannot be duplicated by repeatedly clicking its row. */
  disabled?: boolean;
}

/** The whole picture is a keyboard-accessible choice with visible focus. */
export function SetupChoice({ title, caption, brands, onClick, path, compact = false, disabled }: Props) {
  return (
    <button
      type="button"
      className={compact ? 'setup-choice setup-choice-compact' : 'setup-choice'}
      disabled={disabled}
      data-path={path}
      aria-label={`${title}. Works with ${brands.map((brand) => brand.name).join(', ')}. ${caption}`}
      onClick={onClick}
    >
      <BrandChips brands={brands} />
      <span className="setup-choice-copy">
        <strong>{title}</strong>
        <span>{caption}</span>
      </span>
      <Icon name="chevron" />
    </button>
  );
}
