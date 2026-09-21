/**
 * The section for teams who already pay for browser infrastructure: the same
 * five questions, answered by a CDP SDK and by Oya.
 */
import { COMPARE } from './content';
import styles from '../page.module.css';

/** The comparison, as a table. */
export function Compare() {
  return (
    <section className={styles.compare} aria-labelledby="compare-title">
      <div className={styles.compareIntro}>
        <p className={styles.eyebrow}>Already running browsers in the cloud?</p>
        <h2 id="compare-title">
          Same five questions.
          <br />
          <span>Different answers.</span>
        </h2>
      </div>
      <table className={styles.compareTable}>
        <thead>
          <tr>
            <th scope="col">&nbsp;</th>
            <th scope="col">A cloud browser SDK</th>
            <th scope="col">Oya</th>
          </tr>
        </thead>
        <tbody>
          {COMPARE.map(({ row, others, oya }) => (
            <tr key={row}>
              <th scope="row">{row}</th>
              <td>{others}</td>
              <td className={styles.compareOya}>{oya}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
