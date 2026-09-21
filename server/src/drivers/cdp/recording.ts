/**
 * Recording what a person does in the live view: the channel that streams
 * steps out of the page, and the buffer they collect in.
 */
import { createRequire } from 'module';
import { analyzerSource, getAnalyzer } from './browser-scripts.ts';
import { sessionListener, sessionSender } from './session.ts';
import { MAX_RECORDED_STEPS } from './constants.ts';
import type { CDPDriver } from './driver.ts';

const require = createRequire(import.meta.url);
const { RecordingChannel } = require('../../../../browser/scripts/recording.cjs');

/** Adds newly recorded steps and secret names from the page, skipping duplicates. */
export function collectRecording(driver: CDPDriver, out) {
  if (!driver.recording) return;
  for (const step of out?.steps || []) {
    if (driver.recorded.length >= MAX_RECORDED_STEPS || driver.recordedIds.has(step.id)) continue;
    if (step.id) driver.recordedIds.add(step.id);
    driver.recorded.push(step);
  }
  for (const name of out?.secrets || []) driver.recordedSecrets.add(name);
}

/** Starts streaming recorded steps from the current target. */
export async function armRecording(driver: CDPDriver) {
  driver.recordChannel = new RecordingChannel({
    send: sessionSender(driver, driver.sessionId),
    on: sessionListener(driver, driver.sessionId),
    worldName: driver.worldName,
    analyzer: analyzerSource(getAnalyzer(), driver.tagAttr),
    receive: (out) => driver.collectRecording(out),
  });
  await driver.recordChannel.start();
}
