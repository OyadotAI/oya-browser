/** The control-plane tour's pictures: each one marks the part of the dashboard its step explains. */
import type { ReactNode } from 'react';
import { DashboardPicture } from './setup-dashboard-picture.tsx';
import './setup-illustration.css';

/** One picture per tour step, so a step's copy and its picture cannot drift apart. */
export type SetupScene = 'guide-browser' | 'guide-workspace' | 'guide-page' | 'guide-human';

/** The guarded table keeps every tour step on a known local drawing. */
const SCENES: Record<SetupScene, () => ReactNode> = {
  'guide-browser': () => <DashboardPicture area="browsers" />,
  'guide-workspace': () => <DashboardPicture area="workspace" />,
  'guide-page': () => <DashboardPicture area="page" />,
  'guide-human': () => <DashboardPicture area="human" />,
};

/** Decorative art reserves its aspect ratio; meaningful instructions remain selectable text outside it. */
export function SetupIllustration({
  scene,
}: {
  /** The concept illustrated by the surrounding caption. */ scene: SetupScene;
}) {
  return (
    <svg
      className="setup-illustration"
      viewBox="0 0 320 190"
      width="320"
      height="190"
      aria-hidden="true"
      focusable="false"
      data-scene={scene}
    >
      {Object.hasOwn(SCENES, scene) ? SCENES[scene]() : null}
    </svg>
  );
}
