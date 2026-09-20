/**
 * Recording what a person does in the live view. Every answer is the whole
 * buffer, not a delta, so a poll that never arrives loses nothing.
 */
import type { CDPDriver } from '../driver.ts';
import type { Handler } from './types.ts';

/** Starts, stops or polls the recording, answering with everything recorded so far. */
export const record: Handler = async (driver, params) => {
  await driver.ensureAnalyzer();
  if (params.mode === 'start' && !driver.recording) await startRecording(driver);
  else if (params.mode === 'stop' && driver.recording) await stopRecording(driver);
  else if (driver.recording) await driver.recordChannel?.drain();
  const secrets = [...(driver.recordedSecrets || [])];
  return { ok: true, data: { recording: !!driver.recording, steps: driver.recorded || [], secrets } };
};

/** Clears the buffer, records the starting page, and arms the page. */
async function startRecording(driver: CDPDriver) {
  Object.assign(driver, { recorded: [], recordedSecrets: new Set(), recordedIds: new Set() });
  const url = await driver.evaluate('location.href');
  if (/^https?:\/\//i.test(url || '')) driver.recorded.push({ action: 'navigate', url, start: true, t: Date.now() });
  driver.recording = true;
  await armOrRollBack(driver);
}

/** Arms the recording; if that fails, the driver is not left claiming to record. */
async function armOrRollBack(driver: CDPDriver) {
  try {
    await driver.armRecording();
  } catch (err) {
    driver.recording = false;
    throw err;
  }
}

/** Stops streaming steps; the buffer stays for the answer. */
async function stopRecording(driver: CDPDriver) {
  await driver.recordChannel?.stop();
  driver.recordChannel = null;
  driver.recording = false;
}
