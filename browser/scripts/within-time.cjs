/**
 * A time limit on a promise. Validation uses it so a run never waits forever
 * to start, and the recorder so a page that stops answering (an open alert, a
 * hung renderer) cannot wedge every later start, stop and save behind it.
 */

/** `promise`, or a rejection with `message` once `ms` has passed. */
function withinTime(promise, ms, message) {
  let timer;
  const expired = new Promise((_, reject) => (timer = setTimeout(() => reject(new Error(message)), ms)));
  return Promise.race([promise, expired]).finally(() => clearTimeout(timer));
}

module.exports = { withinTime };
