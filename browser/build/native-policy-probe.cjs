/** Packaging verifies pre-script document and worker execution before claiming native persona protection. */
const PAGE = 'Object.defineProperty(globalThis,"oyaNativePageProbe",{value:42});';
const WORKER = 'Object.defineProperty(globalThis,"oyaNativeWorkerProbe",{value:42});';
/** Policy must install and read back before creating any renderer in this session. */
function preparePolicy(session) {
  if (typeof session._setOyaPreScriptPolicy !== 'function') throw Error('Missing native pre-script protection API');
  session._setOyaPreScriptPolicy({ page: PAGE, worker: WORKER });
  const state = session._getOyaSessionPolicy?.();
  if (
    state?.preScriptPolicyVersion !== 1 ||
    state.preScriptPolicy?.page !== PAGE ||
    state.preScriptPolicy?.worker !== WORKER
  )
    throw Error('Native pre-script policy readback failed');
}
/** Website first script and worker first script must observe policy without debugger or preload assistance. */
async function inspectPolicy(contents) {
  const html = '<script>globalThis.oyaFirstScriptValue=globalThis.oyaNativePageProbe</script>';
  await contents.loadURL('data:text/html,' + encodeURIComponent(html));
  if ((await contents.executeJavaScript('oyaFirstScriptValue')) !== 42)
    throw Error('Native page pre-script ordering failed');
  const result = await contents.executeJavaScript(`new Promise((resolve,reject)=>{
    const url=URL.createObjectURL(new Blob(['postMessage(globalThis.oyaNativeWorkerProbe)'],{type:'text/javascript'}));
    const worker=new Worker(url); worker.onmessage=e=>{worker.terminate();URL.revokeObjectURL(url);resolve(e.data)};
    worker.onerror=e=>{worker.terminate();URL.revokeObjectURL(url);reject(Error(e.message))};
  })`);
  if (result !== 42) throw Error('Native worker pre-script ordering failed');
}
module.exports = { preparePolicy, inspectPolicy };
