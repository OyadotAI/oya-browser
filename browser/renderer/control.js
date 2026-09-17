(() => {
  const status = document.getElementById('control-status');
  const label = document.getElementById('control-label');
  const button = document.getElementById('control-action');
  const resume = document.getElementById('control-resume');
  let current, error = '';
  function render(state) {
    current = state;
    const mine = state.mode === 'human' && state.mine;
    const mode = state.busy || state.taking ? 'taking' : state.mode;
    status.dataset.mode = mode;
    const labels = { agent: 'Agent control', human: mine ? 'You’re in control' : 'Another operator', paused: 'Automation paused', offline: 'Offline · your control', disconnected: 'Disconnected', unavailable: 'Control unavailable', taking: 'Taking control…' };
    label.textContent = error || (state.busyAction === 'return' ? 'Returning control…' : labels[mode]) || 'Checking control…';
    status.title = error || (state.interactive ? 'You can interact with this page.' : 'Page is watch-only. Take control to interact.');
    const available = state.supported || state.localClients > 0 || state.local;
    button.hidden = !available || state.mode === 'human' && !state.mine;
    button.disabled = state.busy || state.taking && !state.mine;
    button.textContent = mine ? 'Release to agent' : 'Take control';
    resume.hidden = state.mode !== 'paused' || state.taking || state.busy;
    resume.disabled = state.busy;
    for (const id of ['btn-reload', 'btn-new-tab', 'btn-record', 'record-toggle']) {
      const element = document.getElementById(id);
      if (element) { element.toggleAttribute('data-control-blocked', !state.interactive); element.setAttribute('aria-disabled', String(!state.interactive)); }
    }
    document.getElementById('url-bar').readOnly = !state.interactive;
  }
  button.addEventListener('click', async () => {
    error = ''; button.disabled = true;
    const result = await oyaBrowser.changeControl(current.mode === 'human' && current.mine ? 'return' : 'acquire');
    error = result.error || '';
    render(result.state);
  });
  resume.addEventListener('click', async () => {
    error = ''; resume.disabled = true;
    const result = await oyaBrowser.changeControl('return');
    error = result.error || ''; render(result.state);
  });
  // Capture prevents local actions before existing navigation/recording listeners.
  document.addEventListener('click', event => {
    if (current?.interactive) return;
    if (event.target.closest('#btn-back, #btn-forward, #btn-reload, #btn-new-tab, .tab-close, #btn-record, #record-toggle')) {
      event.preventDefault(); event.stopImmediatePropagation(); button.focus();
    }
  }, true);
  oyaBrowser.onControlState(state => {
    if (state.mode !== current?.mode || state.revision !== current?.revision || state.connected !== current?.connected) error = '';
    render(state);
  });
  oyaBrowser.getControlState().then(render);
})();
