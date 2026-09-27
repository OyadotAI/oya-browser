/**
 * The walkthrough video behind a poster. Loom's player is heavy and shows its
 * own chrome before it plays, so the page shows a quiet poster in its own
 * style and loads the player only when someone presses play.
 */
'use client';

import { Play } from 'lucide-react';
import { useState } from 'react';
import { WALKTHROUGH } from './content';
import styles from '../page.module.css';

/** The poster, then the player once asked for. */
export function Walkthrough() {
  const [playing, setPlaying] = useState(false);
  if (playing) {
    return (
      <div className={styles.walkthrough}>
        <iframe src={WALKTHROUGH.playing} title={WALKTHROUGH.title} allow="autoplay; fullscreen" allowFullScreen />
      </div>
    );
  }
  return (
    <button type="button" className={`${styles.walkthrough} ${styles.poster}`} onClick={() => setPlaying(true)}>
      <span className={styles.play}>
        <Play size={22} fill="currentColor" aria-hidden="true" />
      </span>
      <span className={styles.posterTitle}>Watch the walkthrough</span>
      <span className={styles.posterMeta}>{WALKTHROUGH.length} · a real portal run, start to replay</span>
    </button>
  );
}
