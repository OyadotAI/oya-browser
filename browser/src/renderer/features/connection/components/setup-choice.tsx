/** Illustrated native buttons show the outcome of each choice before asking for technical details. */
import { Icon } from '../../../ui/index.ts';
import { SetupIllustration, type SetupScene } from './setup-illustration.tsx';
import type { SetupPath } from '../view-models/onboarding-view-model.ts';

/** Choice content stays short while the picture explains the connection. */
interface Props {
  /** Requested path label or recognizable provider name. */
  title: string;
  /** One short explanation that remains accessible without the illustration. */
  caption: string;
  /** A substantial local drawing rather than a standalone icon. */
  scene: SetupScene;
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
export function SetupChoice({ title, caption, scene, onClick, path, compact = false, disabled }: Props) {
  return (
    <button
      type="button"
      className={compact ? 'setup-choice setup-choice-compact' : 'setup-choice'}
      disabled={disabled}
      data-path={path}
      aria-label={`${title}: ${caption}`}
      onClick={onClick}
    >
      <span className="setup-choice-art">
        <SetupIllustration scene={scene} />
      </span>
      <span className="setup-choice-copy">
        <strong>{title}</strong>
        <span>{caption}</span>
      </span>
      <Icon name="chevron" />
    </button>
  );
}
