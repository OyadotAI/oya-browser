/**
 * Evidence: what a page did during a run (console severity, requests,
 * dialogs, downloads), reported as run events without page content.
 */
const { safeUrl } = require('../diagnostics.cjs');

/**
 * An alert has one button, so dismissing it stalled replays that a person
 * had clicked straight through. Accept those; a confirm or prompt is a
 * decision nobody recorded, so it is still refused — but say which, and
 * what it asked, instead of a fixed string.
 */
async function answerDialog(dialog, report) {
  const accept = ['alert', 'beforeunload'].includes(dialog.type());
  const outcome = accept ? 'accepted' : 'dismissed (no recorded checkpoint)';
  report({ kind: 'attention', message: `Browser ${dialog.type()}: "${dialog.message()}" — ${outcome}.` });
  await (accept ? dialog.accept() : dialog.dismiss());
}

/** Reports a console error or warning by severity only. */
function consoleEvent(msg, report) {
  if (!['error', 'warning'].includes(msg.type())) return;
  report({ kind: 'console', level: msg.type(), message: 'Page console ' + msg.type() + ' (message omitted)' });
}

/** Page event → what it reports. */
const PAGE_EVENTS = {
  console: consoleEvent,
  pageerror: (_, report) => report({ kind: 'console', level: 'error', message: 'Uncaught page error' }),
  requestfailed: (request, report) =>
    report({ kind: 'network', url: safeUrl(request.url()), message: 'Request failed' }),
  response: (response, report) => report({ kind: 'network', url: safeUrl(response.url()), status: response.status() }),
  dialog: answerDialog,
  download: (_, report) => report({ kind: 'download', message: 'Download started' }),
};

/** Listens to a page for the rest of the run; each event is tagged with the step running then. */
function attachEvidence(p, emit, state) {
  const report = ({ kind, ...rest }) => emit({ kind, stepId: state.current, ...rest });
  for (const [event, handle] of Object.entries(PAGE_EVENTS)) p.on(event, (value) => handle(value, report));
}

module.exports = { attachEvidence };
