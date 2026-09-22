/**
 * Live view: a browser streams frames only while someone is watching. The
 * registry says when the first viewer arrives and the last one leaves, and the
 * browser's driver does the rest, whatever kind of browser it is.
 *
 * One pair of listeners for the whole process. Registering per browser would
 * leak a listener each time and trip EventEmitter's max at 11 browsers.
 */
import { registry } from './registry.ts';
import { metrics } from '../../platform/metrics.ts';

/** A frame a driver pulled itself; a browser that pushes its own frames is counted where they arrive. */
function frameFrom(id: string) {
  return (dataUrl: string) => {
    registry.pushFrame(id, dataUrl);
    metrics.frames.inc({ client: 'cdp' });
  };
}

registry.on('stream:start', ({ id }) => {
  registry
    .get(id)
    ?.driver.startScreencast(frameFrom(id))
    .catch(() => {});
});

registry.on('stream:stop', ({ id }) => {
  registry
    .get(id)
    ?.driver.stopScreencast()
    .catch(() => {});
});
