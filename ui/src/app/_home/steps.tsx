/**
 * How the product is used, as the kit's steps: three cards in order, each step's result shown
 * in its panel: the portal the agent worked, the playbooks it saved, and the one SDK call a
 * replay is (the kit's code fragment, rewritten).
 */
import type { CSSProperties } from 'react';
import Image from 'next/image';
import { FROM_AGENT, STEPS } from './content';
import { POSTER_HEIGHT, POSTER_WIDTH, SCREENSHOT_HEIGHT, SCREENSHOT_WIDTH, TWO_ON_A_PHONE } from './constants';

/** Where the code fragment sits in its panel: low, rising out of the bottom. */
const CODE_PLACE = { '--fit-max': '380px', '--ay': 'calc(100% - 200px)', '--ay-sm': '16px' } as CSSProperties;

/** The line of the snippet that is the replay itself, marked as the kit marks the line being worked on. */
const MARKED_LINE = 3;

/** The SDK snippet, which is the replay in one call. */
const SDK = FROM_AGENT.find((s) => s.id === 'sdk') ?? FROM_AGENT[0];

/** The kit's code fragment with the SDK call in it, held still. */
function Code() {
  return (
    <div className="panel">
      <div className="stage stage--fade-b">
        <div className="at-rise fit" style={CODE_PLACE}>
          <div className="ui ui-panel ui-code" data-ui="code" data-ui-still>
            <div className="ui-code__tabs">
              <span className="ui-code__tab is-on">
                <i className="ui-code__lang">TS</i>replay.ts
              </span>
              <span className="ui-spacer" />
            </div>
            <pre className="ui-code__pre">
              <code>
                {SDK.code.map((line, i) => (
                  <span key={i} className={i === MARKED_LINE ? 'ui-code__ln is-mark' : 'ui-code__ln'} data-n={i + 1}>
                    {line || ' '}
                  </span>
                ))}
              </code>
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}

/** Each step's panel, in STEPS order. */
const PANELS = [
  <div key="run" className="panel">
    <Image
      className="frame__img"
      src="/prior-auth-poster.jpg"
      alt="An agent on the authorization request page of a payer portal"
      width={POSTER_WIDTH}
      height={POSTER_HEIGHT}
      loading="lazy"
    />
  </div>,
  <div key="save" className="panel">
    <Image
      className="frame__img"
      src="/playbooks-list.jpg"
      alt="The playbooks list in the Oya console"
      width={SCREENSHOT_WIDTH}
      height={SCREENSHOT_HEIGHT}
      loading="lazy"
    />
  </div>,
  <Code key="replay" />,
];

/** The steps section. */
export function Steps() {
  return (
    <section className="section" id="steps" style={TWO_ON_A_PHONE} aria-labelledby="steps-h">
      <div className="wrap">
        <div className="section-head">
          <h2 id="steps-h">
            Set up a prior auth in 3 steps. <span className="muted">Then every case replays.</span>
          </h2>
        </div>
        <div className="grid">
          {STEPS.map(({ n, title, body }, i) => (
            <article key={n} className="card card--row card--open span-4 m-nolift">
              <span className="step-num">{n}</span>
              <h3>{title}</h3>
              <p>{body}</p>
              {PANELS[i]}
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
