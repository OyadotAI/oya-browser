/**
 * The playbooks section: what a saved run is, the real console open on a playbook's
 * generated code in a clean bordered frame, and four cards for what a playbook holds.
 */
import Link from 'next/link';
import Image from 'next/image';
import { SCREENSHOT_HEIGHT, SCREENSHOT_WIDTH, TWO_ON_A_PHONE } from './constants';
import type { CSSProperties } from 'react';

/** What a playbook holds, one name and one line each. */
const PARTS = [
  ['Variables', 'The member, codes and dates you change each run'],
  ['Playwright export', 'Code you can read line by line and keep'],
  ['Healing', 'When a portal changes, the agent fixes that step'],
  ['Routines', 'Runs on a schedule, in your project'],
];

/** The frame's height over its width so the section and its four cells fit cleanly. */
const RATIO = { '--ratio': '36%' } as CSSProperties;

/** Where the screenshot sits in the frame. */
const PLACE = { '--w': '1100px', '--ay': '48px' } as CSSProperties;

/** The playbooks section. */
export function Playbooks() {
  return (
    <section className="section" id="playbooks" style={TWO_ON_A_PHONE} aria-labelledby="playbooks-h">
      <div className="wrap">
        <div className="grid grid--top">
          <h2 className="span-5" id="playbooks-h">
            Every prompt becomes a playbook
          </h2>
          <p className="span-6 start-7 t-lead muted">
            The agent&apos;s run is saved as Playwright code you can read. Inputs become variables, steps become lines,
            and the next run needs no model.{' '}
            <Link className="link" href="/dashboard">
              Open the console
            </Link>
          </p>
        </div>
        <div className="frame frame--hero" style={RATIO}>
          <div className="stage stage--fade-b">
            <Image
              className="at-rise to-edge"
              style={PLACE}
              src="/playbook-open.jpg"
              alt="The Oya console with a playbook open on its generated Playwright file"
              width={SCREENSHOT_WIDTH}
              height={SCREENSHOT_HEIGHT}
              loading="lazy"
            />
          </div>
        </div>
        <div className="grid">
          {PARTS.map(([name, line]) => (
            <div key={name} className="span-3">
              <p className="t-card">{name}</p>
              <p className="muted">{line}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
