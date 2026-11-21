/**
 * The marks on the checklist's ways in: Oya's O, the Gemini and ChatGPT
 * logos so a person spots the plan they already have, and the shell's key icon.
 */
import type { ReactNode } from 'react';
import { Icon } from '../../../ui/index.ts';
import { OPENAI_PATH } from './brand-marks.tsx';
import type { ConnectWay } from '../view-models/checklist-view-model.ts';

/** Oya's O on its own: the logo's green ring, without the orb around it. */
const OYA = (
  <svg className="way-icon" viewBox="0 0 24 24" aria-hidden="true">
    <circle cx="12" cy="12" r="7.5" fill="none" stroke="var(--accent)" strokeWidth="5" />
  </svg>
);

/** Gemini's four-point spark, in its blue-to-violet gradient. */
const GEMINI = (
  <svg className="way-icon" viewBox="0 0 24 24" aria-hidden="true">
    <defs>
      <linearGradient id="way-gemini" x1="0" y1="1" x2="1" y2="0">
        <stop offset="0" stopColor="#4285f4" />
        <stop offset="1" stopColor="#b388ff" />
      </linearGradient>
    </defs>
    <path
      fill="url(#way-gemini)"
      d="M12 2c0 5.5 4.5 10 10 10-5.5 0-10 4.5-10 10 0-5.5-4.5-10-10-10 5.5 0 10-4.5 10-10Z"
    />
  </svg>
);

/** OpenAI's knot, in the text colour. */
const CHATGPT = (
  <svg className="way-icon" viewBox="0 0 24 24" aria-hidden="true">
    <path fill="currentColor" d={OPENAI_PATH} />
  </svg>
);

/** Each way's mark. */
export const WAY_ICONS: Record<ConnectWay, ReactNode> = {
  oya: OYA,
  gemini: GEMINI,
  chatgpt: CHATGPT,
  key: <Icon name="key" />,
};
