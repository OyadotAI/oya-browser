/**
 * The validation worker's bundle entry (out/worker/index.js): the main
 * process forks it as an Electron utility process, and it listens on the
 * port to its parent.
 */
import { WorkflowWorker } from './worker.ts';

new WorkflowWorker(process.parentPort);
