/**
 * Oya against the browser-agent vendors a buyer is also weighing, on the
 * questions a regulated team asks before it signs.
 */
import { COMPARE, RIVALS } from './content';
import { Reveal } from './motion';
import styles from '../page.module.css';

/** The comparison: a table on a wide screen, and one card per question on a phone, each cell labelled with its vendor. */
export function Compare() {
  return (
    <section className={styles.compare} aria-labelledby="compare-title">
      <Reveal className={styles.compareIntro}>
        <p className={styles.eyebrow}>Weighing Browser Use, Anchor or Browserbase?</p>
        <h2 id="compare-title">
          They all replay runs.
          <br />
          <span>Only one keeps your portal work in your network.</span>
        </h2>
      </Reveal>
      <div className={styles.compareScroll}>
        <table className={styles.compareTable}>
          <thead>
            <tr>
              <th scope="col">&nbsp;</th>
              <th scope="col">Oya</th>
              {RIVALS.map((rival) => (
                <th key={rival} scope="col">
                  {rival}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {COMPARE.map(({ row, oya, rivals }) => (
              <tr key={row}>
                <th scope="row">{row}</th>
                <td className={styles.compareOya} data-label="Oya">
                  {oya}
                </td>
                {rivals.map((cell, i) => (
                  <td key={RIVALS[i]} data-label={RIVALS[i]}>
                    {cell}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className={styles.compareNote}>From each vendor&apos;s public site, docs and pricing page, September 2026.</p>
    </section>
  );
}
