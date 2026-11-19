/** The loud coral band that drifts between sections. */
import { BAND } from './bold-content';
import { BAND_COPIES } from './constants';
import styles from './bold.module.css';

/** The band; the words are drawn several times so the loop has no seam on a wide screen, and read out once. */
export function LoudBand() {
  return (
    <div className={styles.drift}>
      {Array.from({ length: BAND_COPIES }, (_, copy) => (
        <ul key={copy} aria-hidden={copy > 0}>
          {BAND.map((word) => (
            <li key={word}>{word}</li>
          ))}
        </ul>
      ))}
    </div>
  );
}
