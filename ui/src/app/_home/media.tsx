/**
 * The section that shows the product running, one clip per claim.
 */
import { MEDIA } from './content';
import styles from '../page.module.css';

/** One claim beside a silent loop of the product doing it. */
function Clip({ item }: { /** The clip and the claim beside it. */ item: (typeof MEDIA)[number] }) {
  return (
    <article className={styles.clip}>
      <div className={styles.clipCopy}>
        <h3>{item.title}</h3>
        <p>{item.body}</p>
        <p className={styles.clipNote}>{item.note}</p>
      </div>
      <video
        className={styles.clipVideo}
        src={item.src}
        poster={item.poster}
        autoPlay
        muted
        loop
        playsInline
        preload="metadata"
        aria-label={item.title}
      />
    </article>
  );
}

/** Both clips. */
export function Media() {
  return (
    <section className={styles.media} aria-labelledby="media-title">
      <div className={styles.mediaIntro}>
        <p className={styles.eyebrow}>See it run</p>
        <h2 id="media-title">
          This is the whole loop.
          <br />
          <span>Ask, save, replay.</span>
        </h2>
      </div>
      {MEDIA.map((item) => (
        <Clip key={item.src} item={item} />
      ))}
    </section>
  );
}
