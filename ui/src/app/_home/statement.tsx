/** A direct time comparison keeps speed claims understandable and separate from audit controls. */

/** Reported prior-authorization times, shown with the same unit and explicit comparison labels. */
const FIGURES = [
  ['By hand', '22–26'],
  ['With Oya', 'Under 2'],
];

/** Presents the existing customer-reported comparison without overlapping speed metrics. */
export function Statement() {
  return (
    <section className="section" id="results" aria-labelledby="results-h">
      <div className="wrap comparison-layout">
        <div>
          <h2 id="results-h">Compare the result with your current process.</h2>
          <p className="deployment-lead">
            Before expanding the workflow, compare time per case with the work your team does today.
          </p>
        </div>
        <div>
          <div role="group" aria-labelledby="prior-auth-time-h">
            <h3 className="comparison-label" id="prior-auth-time-h">
              Time per prior authorization.
            </h3>
            <dl className="comparison">
              {FIGURES.map(([label, value]) => (
                <div key={label}>
                  <dt>{label}</dt>
                  <dd>
                    {value}
                    <span className="comparison-unit"> minutes</span>
                  </dd>
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
