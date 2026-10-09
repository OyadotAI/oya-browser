/** Document-bound recorder lifecycle scripts execute only inside Oya's isolated agent world. */
import { NATIVE_RECORDING } from '../../shared/native-recording.ts';
/** Refuse replacement documents before reading or changing recorder state. */
function guard(documentId: string): string {
  return `const bridge = globalThis.${NATIVE_RECORDING.BINDING};
    if (bridge?.documentId !== ${JSON.stringify(documentId)}) throw new Error('Recording document changed');`;
}
/** Install one recorder owner and a cancellable DOM-ready wait without exposing anything to page globals. */
const START_RECORDER = `(() => { %GUARD%
    if (globalThis.__oyaDocumentRecorder) throw new Error('Document already has a recorder');
    const state = { owner: %OWNER%, armed: false };
    globalThis.__oyaDocumentRecorder = state;
    state.ready = new Promise((resolve, reject) => {
      const cancel = () => {
        if (state.armed) return;
        state.cancelled = true;
        document.removeEventListener('DOMContentLoaded', arm);
        resolve(false);
      };
      const arm = () => {
        if (state.cancelled) return;
        try {
          %ANALYZER%
          window.__acRecordSink = data => bridge.emit(%EPOCH%, JSON.stringify(data));
          window.__acRecordStart();
          state.armed = true;
          resolve(true);
        } catch (error) { reject(error); }
      };
      state.cancel = () => {
        document.removeEventListener('DOMContentLoaded', arm);
        window.removeEventListener('pagehide', cancel);
        cancel();
      };
      window.addEventListener('pagehide', cancel, {once:true});
      if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', arm, {once:true});
      else arm();
    });
    return state.ready;
  })()`;
/** Substitute trusted source last so analyzer text cannot accidentally become a lifecycle placeholder. */
export function startRecorderScript(documentId: string, owner: string, epoch: string, analyzer: string): string {
  return START_RECORDER.replace('%GUARD%', () => guard(documentId))
    .replace('%OWNER%', () => JSON.stringify(owner))
    .replace('%EPOCH%', () => JSON.stringify(epoch))
    .replace('%ANALYZER%', () => analyzer);
}
/** Commands require both the original document and this exact lifecycle owner. */
export function recorderCommandScript(documentId: string, owner: string, command: string): string {
  return `(() => { ${guard(documentId)}
    const state = globalThis.__oyaDocumentRecorder;
    if (!state || state.owner !== ${JSON.stringify(owner)}) throw new Error('Recording owner changed');
    ${command}
  })()`;
}
/** Return final typing directly to the caller rather than racing a last IPC batch against inbox teardown. */
export const STOP_RECORDER = `state.cancel();
  window.__acRecordSink = undefined;
  try { if (state.armed) { window.__acRecordStop(); return window.__acRecordDrain(true); } }
  finally { window.__acRecordSink = undefined; delete globalThis.__oyaDocumentRecorder; }`;
