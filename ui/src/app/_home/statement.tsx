/** A direct time comparison keeps speed claims understandable and separate from audit controls. */

/** Reported prior-authorization times, shown with the same unit and explicit comparison labels. */
const FIGURES = [
  ['By hand', '22–26 minutes'],
  ['With Oya', 'Under 2 minutes'],
];

/** Presents the existing customer-reported comparison without overlapping speed metrics. */
export function Statement() {
  return (
    <section className="section" id="results" aria-labelledby="results-h">
      <div className="wrap comparison-layout">
        <h2 id="results-h">See what customers report.</h2>
        <div>
          <div role="group" aria-labelledby="prior-auth-time-h">
            <h3 className="comparison-label" id="prior-auth-time-h">
              Time per prior authorization.
            </h3>
            <dl className="comparison">
              {FIGURES.map(([label, value]) => (
                <div key={label}>
                  <dt>{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
          </div>
          <div className="comparison-note" role="group" aria-labelledby="throughput-h">
            <h3 id="throughput-h">Reported throughput</h3>
            <p>Customers report completing 10 to 30 times as many tasks per hour as they could manually.</p>
          </div>
        </div>
      </div>
    </section>
  );
}
