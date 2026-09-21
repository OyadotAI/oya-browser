/**
 * The recorder bootstrap evaluated in the page, and the expression that stops
 * it. Their text is what the page runs, so they keep their exact shape.
 */

/**
 * The bootstrap evaluated in each recorder world. %BINDING% and %ANALYZER% are
 * filled in by recorderSource(); the text is what the page runs, so it keeps
 * its exact shape.
 */
const RECORDER_TEMPLATE = `(() => {
        window.__acRecordCancelled = false;
        window.__acRecordSink = data => window[%BINDING%](JSON.stringify(data));
        const arm = () => {
          if (window.__acRecordCancelled) return;
          %ANALYZER%
          window.__acRecordStart();
        };
        if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', arm, { once: true });
        else arm();
      })();`;

/** Tells every recorder to stop, drops its sink, and drains what it still holds. */
const STOP_EXPRESSION = `window.__acRecordCancelled = true;
        window.__acRecordStop?.();
        window.__acRecordSink = undefined;
        window.__acRecordDrain?.(true);`;

/** The recorder bootstrap for this channel's binding and analyzer source. */
function recorderSource(binding, analyzer) {
  return RECORDER_TEMPLATE.replace('%BINDING%', () => JSON.stringify(binding)).replace('%ANALYZER%', () => analyzer);
}

module.exports = { recorderSource, STOP_EXPRESSION };
