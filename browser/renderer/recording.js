(() => {
  const $ = id => document.getElementById(id);
  let state, selected, busy = false, tab = 'steps', expanded = false, stepsSignature = '', editorSignature = '', variablesSignature = '', librarySignature = '', runHistorySignature = '', codeSignature = '';
  const names = { navigate: 'Navigate', type: 'Fill', click: 'Click', select_option: 'Select', press_key: 'Press key', scroll: 'Scroll', upload_file: 'Upload', wait: 'Wait for element', assert_visible: 'Assert visible', assert_text: 'Assert text', assert_value: 'Assert value', assert_url: 'Assert URL', checkpoint: 'Human checkpoint' };
  const say = (message = '', error = false) => { $('record-result').textContent = message; $('record-result').classList.toggle('error', error); };
  const node = (tag, text, cls) => { const el = document.createElement(tag); if (text != null) el.textContent = text; if (cls) el.className = cls; return el; };
  function button(text, fn, title = text) { const el = node('button', text, 'text-button'); el.type = 'button'; el.title = title; el.addEventListener('click', fn); return el; }
  async function command(cmd) {
    try { const next = await oyaBrowser.workspace(cmd); render(next); return next; }
    catch (error) { say(error.message.replace(/^Error invoking remote method '[^']+': Error: /, ''), true); }
  }
  function selectTab(value) {
    tab = value;
    document.querySelectorAll('[data-studio]').forEach(el => { const active = el.dataset.studio === tab; el.setAttribute('aria-selected', active); el.tabIndex = active ? 0 : -1; });
    for (const name of ['steps', 'run', 'inspect']) $('studio-' + name).hidden = name !== tab;
  }
  document.querySelectorAll('[data-studio]').forEach(el => {
    el.addEventListener('click', () => selectTab(el.dataset.studio));
    el.addEventListener('keydown', event => { if (!['ArrowLeft', 'ArrowRight'].includes(event.key)) return; event.preventDefault(); const tabs = ['steps', 'run', 'inspect']; selectTab(tabs[(tabs.indexOf(tab) + (event.key === 'ArrowRight' ? 1 : 2)) % 3]); document.querySelector(`[data-studio="${tab}"]`).focus(); });
  });
  function field(label, value, change, type = 'text') {
    const group = node('label', null, 'studio-field'); group.append(node('span', label));
    const input = node('input'); input.type = type; input.value = value ?? ''; input.addEventListener('change', () => change(type === 'number' ? Number(input.value) : input.value)); group.append(input); return group;
  }
  function render(next) {
    if (!next?.draft) return; state = next;
    const d = state.draft, recording = d.phase === 'recording', active = ['starting', 'running', 'paused', 'stopping'].includes(state.run?.status);
    $('record-title').textContent = recording ? 'Recording your workflow.' : d.steps.length ? 'Shape your workflow.' : 'Make it repeatable.';
    $('record-count').textContent = `${d.steps.length} / 500`;
    $('draft-status').textContent = state.storageError || (recording ? 'Recording · pauses safely on restart · pages and identifiable frames' : 'Saved on this device · encrypted with your OS keychain');
    $('draft-status').classList.toggle('error', !!state.storageError);
    $('record-toggle').textContent = recording ? 'Finish recording' : d.steps.length ? 'Resume recording' : 'Start recording';
    $('record-toggle').disabled = busy || active;
    $('btn-record').classList.toggle('recording', recording);
    $('btn-record').innerHTML = shellIcon(recording ? 'stop' : 'record') + `<span class="button-label">${recording ? 'Pause recording' : 'Record'}</span>`;
    $('record-validate').disabled = recording || active || !d.steps.length || state.issues.length > 0;
    $('record-clear').disabled = busy || recording || active;
    $('draft-library').disabled = busy || recording || active;
    $('record-save').disabled = busy || recording || active || !connected || !d.steps.length || state.issues.length > 0;
    $('record-save').title = connected ? 'Save this playbook to Oya' : 'Connect to a server to publish. Local drafts and export work offline.';
    $('record-finish').hidden = recording || !d.steps.length;
    $('record-finish-title').textContent = d.publishedAt ? 'Playbook saved to Oya' : 'Recording captured';
    $('record-finish-copy').textContent = d.publishedAt ? 'Your local draft is kept too. Save again after making changes.' : state.storageError ? 'Your recording is in memory. Save or export it before closing Oya.' : `${d.steps.length} steps saved on this device. Name your workflow and save a playbook to reuse it in Oya.`;
    $('record-save').textContent = busy && !recording ? 'Saving…' : d.publishedAt ? 'Save changes' : 'Save playbook';
    $('record-save-hint').textContent = !connected ? 'Connect this browser to save to Oya. Your local draft and Playwright export are available now.' : state.issues.length ? 'Resolve the highlighted steps before saving.' : 'Saves to your connected Oya workspace.';
    $('step-undo').disabled = recording || active || !state.canUndo; $('step-redo').disabled = recording || active || !state.canRedo; $('step-add').disabled = recording || active;
    const libraryKey = JSON.stringify(state.library.map(item => [item.id, item.name, item.error]));
    if (libraryKey !== librarySignature) { librarySignature = libraryKey; $('draft-library').replaceChildren(...state.library.map(item => { const option = node('option', item.name + (item.error ? ' · recovery unavailable' : '')); option.value = item.id; option.disabled = !!item.error; return option; })); }
    $('draft-library').value = d.id;
    if (document.activeElement !== $('record-name')) $('record-name').value = d.name === 'Untitled workflow' ? '' : d.name;
    if (document.activeElement !== $('record-desc')) $('record-desc').value = d.description;
    if (!d.steps.some(s => s.id === selected)) selected = d.steps[0]?.id;
    const signature = JSON.stringify([d.steps, selected, recording, active]);
    if (signature !== stepsSignature) {
      stepsSignature = signature; const list = $('record-steps'), scroll = list.scrollTop;
      list.replaceChildren();
      if (!d.steps.length) { const empty = node('div', null, 'record-empty'); empty.append(node('h3', 'A workflow starts with you.'), node('p', 'Record the actions you perform, pause to refine them, then validate the exact Playwright code. Hidden fields are excluded.')); list.append(empty); }
      d.steps.forEach((step, index) => {
        const row = node('button', null, 'studio-step' + (selected === step.id ? ' selected' : '') + (!step.enabled ? ' disabled' : ''));
        row.type = 'button'; row.setAttribute('aria-pressed', selected === step.id);
        const copy = node('span', null, 'step-copy'); copy.append(node('strong', names[step.action] || step.action), node('span', step.candidates?.[0]?.value || step.url || step.key || 'Select to configure', 'step-value'));
        row.append(node('span', step.breakpoint ? '●' : String(index + 1).padStart(2, '0'), 'step-number'), copy);
        if (state.issues.some(issue => issue.stepId === step.id)) row.append(node('span', '!', 'step-issue'));
        row.addEventListener('click', () => { selected = step.id; render(state); }); list.append(row);
      }); list.scrollTop = scroll;
    }
    renderEditor(d.steps.find(s => s.id === selected), busy || recording || active);
    renderVariables(d.variables);
    const nextCodeSignature = state.code + ':' + selected;
    if (nextCodeSignature !== codeSignature) {
      codeSignature = nextCodeSignature;
      $('record-code').replaceChildren(...(state.code || 'Resolve the capture issues before exporting this workflow.').split('\n').map((line, index) => {
        const el = node('span', null, 'code-line' + (state.mapping?.[selected] === index + 1 ? ' selected' : ''));
        const number = node('span', String(index + 1), 'code-line-number'); number.setAttribute('aria-hidden', 'true');
        el.append(number, document.createTextNode(line + '\n')); return el;
      }));
    }
    $('inspect-issues').replaceChildren(...state.issues.map(issue => node('p', issue.message, 'studio-issue')));
    renderRun();
  }
  function renderEditor(step, locked) {
    const signature = JSON.stringify([step, locked]); if (signature === editorSignature) return; editorSignature = signature;
    const host = $('step-editor'); host.hidden = !step; host.replaceChildren(); if (!step) return;
    const patch = value => command({ type: 'update', id: step.id, patch: value });
    const header = node('div', null, 'editor-heading'); header.append(node('h3', 'Step ' + (state.draft.steps.indexOf(step) + 1)), node('span', names[step.action] || step.action, 'eyebrow')); host.append(header);
    const controls = node('div', null, 'studio-toolbar');
    controls.append(button(step.enabled ? 'Disable' : 'Enable', () => patch({ enabled: !step.enabled })), button(step.breakpoint ? 'Remove breakpoint' : 'Breakpoint', () => patch({ breakpoint: !step.breakpoint })), button('↑', () => command({ type: 'move', id: step.id, delta: -1 }), 'Move step up'), button('↓', () => command({ type: 'move', id: step.id, delta: 1 }), 'Move step down'), button('Duplicate', () => command({ type: 'duplicate', id: step.id })), button('Delete', () => command({ type: 'delete', id: step.id })));
    host.append(controls);
    if (['click', 'type', 'select_option', 'upload_file', 'wait', 'assert_visible', 'assert_text', 'assert_value'].includes(step.action)) {
      const target = step.candidates?.[0] || { kind: 'css', value: '' };
      const group = node('label', null, 'studio-field'); group.append(node('span', 'Target strategy')); const select = node('select');
      for (const kind of ['testId', 'role', 'label', 'text', 'placeholder', 'css']) { const option = node('option', kind); option.value = kind; select.append(option); } select.value = target.kind;
      select.addEventListener('change', () => patch({ candidates: [{ ...target, kind: select.value }, ...step.candidates.slice(1)] })); group.append(select); host.append(group);
      host.append(field('Target', target.value, value => patch({ candidates: [{ ...target, value }, ...step.candidates.slice(1)] })));
      if (target.kind === 'role') host.append(field('ARIA role', target.role || 'button', role => patch({ candidates: [{ ...target, role }, ...step.candidates.slice(1)] })));
      host.append(button('Pick target on page', async () => { say('Click an element in the page inspector. Escape cancels.'); await command({ type: 'pick', id: step.id }); }));
      if (step.candidates.length > 1) { const alternatives = node('details', null, 'studio-details'); alternatives.append(node('summary', `${step.candidates.length - 1} recorded alternatives`)); step.candidates.slice(1).forEach(c => alternatives.append(button(`${c.kind}: ${c.value}`, () => patch({ candidates: [c, ...step.candidates.filter(item => item !== c)] })))); host.append(alternatives); }
    }
    const valueKey = ({ navigate: 'url', type: 'text', select_option: 'option', upload_file: 'file', press_key: 'key', assert_url: 'expected', assert_text: 'expected', assert_value: 'expected' })[step.action];
    if (valueKey) host.append(field(valueKey === 'expected' ? 'Expected result' : valueKey === 'text' ? 'Value or {{variable}}' : valueKey, step[valueKey], value => patch({ [valueKey]: value })));
    host.append(field('Frame selectors (one per level, separated by →)', (step.frames || []).join(' → '), value => patch({ frames: value.split('→').map(s => s.trim()).filter(Boolean) })));
    host.append(field('Timeout (milliseconds)', step.timeout, timeout => patch({ timeout }), 'number'));
    if (step.captureIssue) host.append(node('p', step.captureIssue, 'studio-issue'));
    host.append(button('Run to this step', () => validate({ runTo: step.id })));
    host.querySelectorAll('button,input,select').forEach(el => { el.disabled = locked; });
  }
  function renderVariables(variables) {
    const signature = JSON.stringify([variables, state.draft.steps.map(s => [s.text, s.url, s.expected, s.file, s.option])]); if (signature === variablesSignature) return; variablesSignature = signature;
    const host = $('workflow-variables'); host.replaceChildren();
    for (const [name, config] of Object.entries(variables)) {
      const row = node('div', null, 'variable-row'); row.append(field('Variable name', name, nextName => command({ type: 'rename-variable', name, nextName })));
      const secret = node('label', null, 'studio-checkbox'), check = node('input'); check.type = 'checkbox'; check.checked = !!config.secret;
      check.addEventListener('change', () => command({ type: 'variables', variables: { ...variables, [name]: { ...config, secret: check.checked, ...(check.checked ? { default: undefined } : {}) } } })); secret.append(check, node('span', 'Secret')); row.append(secret);
      if (!config.secret) row.append(field('Default', config.default || '', value => command({ type: 'variables', variables: { ...variables, [name]: { ...config, default: value } } })));
      row.append(button('Remove', () => { const next = { ...variables }; delete next[name]; command({ type: 'variables', variables: next }); })); host.append(row);
    }
    const used = [...new Set([...JSON.stringify(state.draft.steps).matchAll(/\{\{([A-Za-z_]\w*)\}\}/g)].map(m => m[1]))];
    $('run-inputs').replaceChildren(...used.map(name => { const el = field(`${name}${variables[name]?.secret ? ' · secret' : ''}`, variables[name]?.default || '', () => {}, variables[name]?.secret ? 'password' : 'text'); el.querySelector('input').dataset.variable = name; el.querySelector('input').autocomplete = 'off'; return el; }));
  }
  function renderRun() {
    const run = state.run;
    const signature = JSON.stringify(state.runHistory);
    if (signature !== runHistorySignature) { runHistorySignature = signature; const initial = node('option', 'Previous runs'); initial.value = ''; $('run-history').replaceChildren(initial, ...(state.runHistory || []).map(item => { const option = node('option', item.name + ' · ' + new Date(item.updatedAt).toLocaleString()); option.value = item.id; return option; })); }
    $('run-history').value = run?.id || '';
    $('run-history').disabled = ['starting', 'running', 'paused', 'stopping'].includes(run?.status);

    $('run-title').textContent = run ? ({ succeeded: 'Steps completed.', failed: 'Needs your attention.', 'outcome-unknown': 'Check the website before retrying.', paused: 'Paused at a step.', running: 'Validating workflow…', starting: 'Preparing a fresh tab…', interrupted: 'Run interrupted.', stopping: 'Stopping…', stopped: 'Run stopped.' })[run.status] || run.status : 'Ready when you are.';
    $('run-summary').textContent = run?.error || (run?.status === 'succeeded' ? run.assertions ? `${run.assertions} assertions passed.` : 'Actions completed. Add assertions to verify the outcome.' : 'Validation uses your current login and can change real data.');
    document.querySelectorAll('[data-run]').forEach(el => { el.disabled = !run || !['starting', 'running', 'paused'].includes(run.status); });
    const events = (run?.events || []).filter(e => e.kind === 'step' || e.kind === 'attention' || e.kind === 'target').slice(-100);
    $('run-events').replaceChildren(...events.map(event => { const item = node('div', null, 'run-event ' + (event.status || '')); const index = state.draft.steps.findIndex(step => step.id === event.stepId); item.append(node('span', index < 0 ? '•' : String(index + 1).padStart(2, '0'), 'step-number'), node('span', event.message || event.status || event.kind), node('small', event.duration != null ? event.duration + ' ms' : '')); if (index >= 0) { item.tabIndex = 0; const go = () => { selected = event.stepId; selectTab('steps'); render(state); }; item.addEventListener('click', go); item.addEventListener('keydown', e => { if (e.key === 'Enter') go(); }); } return item; }));
    $('run-repairs').replaceChildren(...(run?.repairs || []).map(repair => { const item = node('div', null, 'repair-review'); item.append(node('strong', 'Repair draft ready'), node('p', `${repair.original.kind} → ${repair.replacement.kind}. The original draft is unchanged.`), button('Review repair', () => command({ type: 'open', id: repair.draftId }).then(() => selectTab('steps')))); return item; }));
    // Preview is the same centrally redacted object written by the main process.
    $('support-preview').textContent = JSON.stringify(state.support || {}, null, 2);
  }
  async function toggleRecording() {
    if (busy) return; busy = true;
    try { if (state?.draft.phase === 'recording') { await oyaBrowser.stopRecording(); selectTab('steps'); requestAnimationFrame(() => { $('record-finish').scrollIntoView({ block: 'nearest' }); if (!$('record-name').value) $('record-name').focus({ preventScroll: true }); }); } else if (state?.draft.steps.length) await command({ type: 'resume-recording' }); else await oyaBrowser.startRecording(); say(); await command({ type: 'get' }); }
    catch (error) { say(error.message, true); } finally { busy = false; if (state) render(state); }
  }
  async function validate(extra = {}) { selectTab('run'); const vars = Object.fromEntries([...$('run-inputs').querySelectorAll('input')].map(input => [input.dataset.variable, input.value])); await command({ type: 'validate', vars, ...extra }); $('run-inputs').querySelectorAll('input[type=password]').forEach(input => { input.value = ''; }); }
  $('record-toggle').addEventListener('click', toggleRecording);
  $('btn-record').addEventListener('click', async () => { if (!devOpen) await oyaBrowser.toggleDevPanel(); showDevPane('record'); selectTab('steps'); await toggleRecording(); });
  $('run-history').addEventListener('change', e => { if (e.target.value) command({ type: 'open-run', id: e.target.value }); });
  $('record-validate').addEventListener('click', () => validate());
  $('record-clear').addEventListener('click', () => command({ type: 'new' }));
  $('draft-library').addEventListener('change', e => command({ type: 'open', id: e.target.value }));
  $('studio-expand').addEventListener('click', async () => { expanded = !expanded; await oyaBrowser.resizeDevPanel(expanded ? 560 : 360); $('studio-expand').textContent = expanded ? 'Compact' : 'Expand'; });
  $('step-undo').addEventListener('click', () => command({ type: 'undo' })); $('step-redo').addEventListener('click', () => command({ type: 'redo' }));
  $('step-add').addEventListener('change', async e => { if (!e.target.value) return; await command({ type: 'add', id: selected, step: { action: e.target.value, candidates: [], expected: '' } }); e.target.value = ''; selected = state?.draft.steps.find((step, i, list) => i > 0 && list[i - 1].id === selected)?.id || state?.draft.steps.at(-1)?.id; render(state); });
  $('variable-add').addEventListener('click', async () => { const variables = { ...state.draft.variables }; let n = 1; while (variables['input_' + n]) n++; variables['input_' + n] = { default: '' }; await command({ type: 'variables', variables }); say(`Use {{input_${n}}} in a step. Rename inputs by editing the placeholder and adding its matching variable.`); });
  for (const id of ['record-name', 'record-desc']) $(id).addEventListener('change', () => command({ type: 'metadata', name: $('record-name').value || 'Untitled workflow', description: $('record-desc').value }));
  document.querySelectorAll('[data-run]').forEach(el => el.addEventListener('click', () => command({ type: 'control', command: el.dataset.run })));
  $('record-name').addEventListener('keydown', event => { if (event.key === 'Enter' && !$('record-save').disabled) { event.preventDefault(); $('record-name').blur(); $('record-save').click(); } });
  $('record-save').addEventListener('click', async () => { if (busy) return; busy = true; render(state); try { const name = $('record-name').value.trim(); if (!/^[\w-]{1,64}$/.test(name)) throw new Error('Use 1–64 letters, numbers, hyphens or underscores for the published name.'); const result = await oyaBrowser.saveRecording(name, $('record-desc').value || name); if (result.error) throw new Error(result.error); say(`Playbook “${name}” saved to Oya. Your local draft is retained.`); await command({ type: 'get' }); } catch (error) { say(error.message, true); } finally { busy = false; render(state); } });
  $('record-copy').addEventListener('click', async () => { try { await navigator.clipboard.writeText(state.code); say('Playwright module copied.'); } catch (error) { say(error.message, true); } });
  $('record-download').addEventListener('click', async () => { try { if (!state.code) throw new Error('Resolve the capture issues first.'); const result = await oyaBrowser.exportPlaywright({ name: state.draft.name, code: state.code }); if (result.saved) say('Playwright module exported.'); } catch (error) { say(error.message, true); } });
  $('support-export').addEventListener('click', () => command({ type: 'support' }));
  oyaBrowser.onWorkspace(render); oyaBrowser.onWsStatus(() => { if (state) render(state); });
  command({ type: 'get' }); selectTab('steps');
})();
