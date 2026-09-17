(() => {
  const $ = id => document.getElementById(id);
  const toggle = $('record-toggle'), stepsEl = $('record-steps'), save = $('record-save');
  let state = { recording: false, steps: [] }, phase = 'idle', busy = false, lastSteps = '', savedName = '';
  const names = { navigate: 'Navigate', type: 'Fill', click: 'Click', select_option: 'Select', press_key: 'Press key', scroll: 'Scroll', upload_file: 'Upload' };
  function say(message = '', error = false) {
    $('record-result').textContent = message; $('record-result').classList.toggle('error', error);
  }
  function render(next = state) {
    state = next;
    const steps = state.steps || [];
    toggle.disabled = busy; save.disabled = busy || !steps.length || !connected;
    $('record-clear').disabled = busy || !steps.length;
    $('btn-record').disabled = busy;
    $('btn-record').classList.toggle('recording', state.recording);
    $('btn-record').innerHTML = shellIcon(state.recording ? 'stop' : 'record') + `<span class="button-label">${state.recording ? 'Stop recording' : 'Record'}</span>`;
    toggle.innerHTML = shellIcon(state.recording ? 'stop' : 'record') + (state.recording ? 'Stop recording' : steps.length ? 'New recording' : 'Start recording');
    $('record-title').textContent = state.recording ? 'Recording your workflow.' : phase === 'saved' ? 'Ready to run again.' : steps.length ? 'Review your workflow.' : 'Make it repeatable.';
    $('record-count').textContent = steps.length + (steps.length === 1 ? ' step' : ' steps');
    $('record-footer').hidden = !steps.length || state.recording || phase === 'saved';
    save.textContent = phase === 'saving' ? 'Saving…' : phase === 'failed' ? 'Retry save' : 'Save playbook';
    save.title = connected ? '' : 'Connect this browser to save the playbook';
    const signature = JSON.stringify(steps);
    if (signature === lastSteps) return;
    lastSteps = signature;
    const follow = stepsEl.scrollHeight - stepsEl.scrollTop - stepsEl.clientHeight < 48;
    const expanded = new Set([...stepsEl.querySelectorAll('details[open]')].map(el => el.dataset.index));
    const position = stepsEl.scrollTop;
    stepsEl.replaceChildren();
    if (!steps.length) {
      stepsEl.innerHTML = `<div class="record-empty"><span class="empty-icon">${shellIcon('record')}</span><h3>Do it once. Keep the workflow.</h3><p>Start recording, then browse as usual. Review your actions and save a reusable playbook with Playwright code.</p></div>`;
    }
    steps.forEach((step, index) => {
      const label = step.el?.text || step.el?.ariaLabel || step.el?.placeholder || step.el?.name || step.el?.domId || step.el?.tag || '';
      const value = step.action === 'navigate' ? step.url : step.action === 'type' ? step.text : step.action === 'select_option' ? step.option : step.key || '';
      const item = document.createElement('details'); item.className = 'step'; item.dataset.index = String(index); item.open = expanded.has(String(index));
      const summary = document.createElement('summary');
      const number = document.createElement('span'); number.className = 'step-number'; number.textContent = index + 1;
      const copy = document.createElement('span'); copy.className = 'step-copy';
      const title = document.createElement('strong'); title.textContent = (names[step.action] || step.action) + (label ? ' · ' + label : '');
      const detail = document.createElement('span'); detail.className = 'step-value'; detail.textContent = value === '' && step.action === 'type' ? 'Clear field' : value || 'View target details';
      const arrow = document.createElement('span'); arrow.className = 'step-chevron'; arrow.innerHTML = shellIcon('chevron');
      copy.append(title, detail); summary.append(number, copy, arrow);
      const metadata = document.createElement('pre'); metadata.textContent = JSON.stringify({ action: step.action, ...(step.el ? { target: step.el } : {}), ...(value ? { value } : {}) }, null, 2);
      item.append(summary, metadata); stepsEl.append(item);
    });
    stepsEl.scrollTop = follow ? stepsEl.scrollHeight : position;
  }
  async function changeRecording() {
    if (busy) return;
    const starting = !state.recording;
    busy = true; render();
    try {
      if (starting && state.steps.length && phase !== 'saved' && !await oyaBrowser.confirmDiscardRecording()) return;
      say();
      const next = starting ? await oyaBrowser.startRecording() : await oyaBrowser.stopRecording();
      if (starting) { $('record-export').hidden = true; $('record-code').textContent = ''; savedName = ''; }
      phase = next.recording ? 'recording' : 'stopped'; render(next);
      if (!next.recording && !connected) say('Recording kept here. Connect this browser to save your playbook.');
    } catch (error) { say(error.message || 'Could not change recording state. Try again.', true); }
    finally { busy = false; render(); }
  }
  $('btn-record').addEventListener('click', async () => {
    if (!devOpen) await oyaBrowser.toggleDevPanel(); showDevPane('record'); await changeRecording();
  });
  toggle.addEventListener('click', changeRecording);
  $('record-clear').addEventListener('click', async () => {
    if (busy) return;
    busy = true; render();
    try {
      if (!await oyaBrowser.confirmDiscardRecording()) return;
      if (state.recording) await oyaBrowser.stopRecording();
      const next = await oyaBrowser.clearRecording(); phase = 'idle'; savedName = ''; $('record-export').hidden = true; say(); render(next);
    } catch (error) { say(error.message || 'Could not discard the recording.', true); }
    finally { busy = false; render(); }
  });
  save.addEventListener('click', async () => {
    if (busy) return;
    const name = $('record-name').value.trim();
    if (!/^[\w-]{1,64}$/.test(name)) { say('Use 1–64 letters, numbers, hyphens or underscores for the name.', true); $('record-name').focus(); return; }
    phase = 'saving'; busy = true; say(); render();
    try {
      const result = await oyaBrowser.saveRecording(name, $('record-desc').value.trim() || name);
      if (result.error) throw new Error(result.error);
      savedName = result.name || name; phase = 'saved';
      $('record-code').textContent = result.code || '';
      $('record-export').hidden = !result.code;
      say(`Saved “${savedName}”. Your Playwright script is ready.`);
    } catch (error) { phase = 'failed'; say(error.message || 'Could not save. Your recording is still here.', true); }
    finally { busy = false; render(); }
  });
  $('record-copy').addEventListener('click', async () => {
    try { await navigator.clipboard.writeText($('record-code').textContent); say('Playwright code copied.'); }
    catch { say('Could not copy. Expand the script and select the code to copy it manually.', true); $('record-export').open = true; $('record-code').focus(); }
  });
  $('record-download').addEventListener('click', async () => {
    try { const result = await oyaBrowser.exportPlaywright({ name: savedName, code: $('record-code').textContent }); if (result.saved) say('Playwright script saved.'); }
    catch (error) { say(error.message || 'Could not save the script. Try again.', true); }
  });
  oyaBrowser.onRecordedSteps(next => {
    if (phase === 'saved' && !next.recording && !next.steps?.length) return;
    if (!busy) phase = next.recording ? 'recording' : next.steps?.length ? 'stopped' : 'idle';
    render(next);
  });
  oyaBrowser.onWsStatus(() => render());
  render();
})();
