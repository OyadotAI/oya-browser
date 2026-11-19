/** A real, silent product recording introduces the task before the separate interactive simulation. */
'use client';
import { POSTER_HEIGHT, POSTER_WIDTH } from './constants';
import { useRecordedDemo, VIDEO_MESSAGES } from './use-recorded-demo';
import { ArrowUpRight } from 'lucide-react';
import styles from './recorded-demo.module.css';

/** The shared recovery controller leaves native media playback in charge. */
type Player = ReturnType<typeof useRecordedDemo>;

/** Intrinsic dimensions reserve space even when playback is unavailable or deferred. */
function Recording({ attach, failed, loaded }: Pick<Player, 'attach' | 'failed' | 'loaded'>) {
  return (
    <video
      ref={attach}
      src="/prior-auth.mp4"
      poster="/prior-auth-poster.jpg"
      width={POSTER_WIDTH}
      height={POSTER_HEIGHT}
      controls
      autoPlay
      loop
      muted
      playsInline
      preload="metadata"
      onError={failed}
      onLoadedMetadata={loaded}
      aria-label="Recorded demo: Oya files a prior authorization on a mock payer portal"
    >
      Your browser cannot play this recording. <a href="#workflows">Explore the interactive walkthrough.</a>
    </video>
  );
}

/** Recovery persists after the first failure so retries keep their keyboard focus. */
function Recovery({ state, retry }: Pick<Player, 'state' | 'retry'>) {
  return (
    <div className={styles.recovery} hidden={state === 'ready'}>
      <p role="status">{VIDEO_MESSAGES[state]}</p>
      <button className={`link ${styles.retry}`} type="button" onClick={retry}>
        Try again
      </button>
    </div>
  );
}

/** Visitors can watch the muted recording or continue to the simulation without signing in. */
export function RecordedDemo() {
  const { attach, failed, loaded, state, retry } = useRecordedDemo();
  return (
    <section className={styles.section} id="recorded-demo" aria-label="Product demo video">
      <figure className={styles.recording}>
        <div className={styles.chassis}>
          <Recording attach={attach} failed={failed} loaded={loaded} />
        </div>
        <Recovery state={state} retry={retry} />
        <div className={styles.links}>
          <a className="link" href="#workflows">
            Explore walkthrough <ArrowUpRight aria-hidden="true" />
          </a>
        </div>
      </figure>
    </section>
  );
}
