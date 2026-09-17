
    const body = document.body;
    const urlBar = document.getElementById('url-bar');
    const connPill = document.getElementById('conn-pill');
    const connLabel = document.getElementById('conn-label');
    const errorEl = document.getElementById('setup-error');

    let connected = false;

    // ── Setup screen actions ──

    document.getElementById('btn-connect').addEventListener('click', async () => {
      const server = document.getElementById('cfg-server').value.trim();
      const key = document.getElementById('cfg-key').value.trim();
      const name = document.getElementById('cfg-name').value.trim();

      if (!/^wss?:\/\/[^\s]+$/.test(server)) { errorEl.textContent = 'Enter a valid ws:// or wss:// server address.'; return; }
      if (!key) { errorEl.textContent = 'API key is required'; return; }
      errorEl.textContent = '';

      const btn = document.getElementById('btn-connect');
      btn.disabled = true;
      btn.textContent = 'Connecting...';

      try { await oyaBrowser.saveConfig({ serverUrl: server, apiKey: key, browserName: name || undefined }); }
      catch (error) { errorEl.textContent = error.message || 'Could not save connection settings.'; btn.disabled = false; btn.textContent = 'Connect'; return; }

      // Wait for ws-status callback to switch modes, but timeout after 5s
      setTimeout(() => {
        btn.disabled = false;
        btn.textContent = 'Connect';
        if (!connected) {
          errorEl.textContent = 'Could not connect — check URL and API key';
        }
      }, 5000);
    });

    document.getElementById('btn-skip').addEventListener('click', () => {
      oyaBrowser.enterBrowsing();
    });

    // Enter in setup fields triggers connect
    document.querySelectorAll('.setup-card input').forEach(input => {
      input.addEventListener('keydown', e => {
        if (e.key === 'Enter') document.getElementById('btn-connect').click();
      });
    });

    // ── Toolbar actions ──

    urlBar.addEventListener('keydown', e => {
      if (e.key === 'Enter') {
        oyaBrowser.navigate(urlBar.value.trim());
        urlBar.blur();
      }
    });

    document.getElementById('btn-back').addEventListener('click', () => oyaBrowser.goBack());
    document.getElementById('btn-forward').addEventListener('click', () => oyaBrowser.goForward());
    document.getElementById('btn-reload').addEventListener('click', () => oyaBrowser.reload());

    oyaBrowser.onUrlChanged(url => { urlBar.value = url; });
    oyaBrowser.onTitleChanged(title => { document.title = title ? `${title} — Oya Browser` : 'Oya Browser'; });

    // ── Updates ──
    //
    // One control does three jobs: it is the version label, the "check now"
    // button, and — once an update is staged — the restart button. The update
    // installs on quit regardless, so nothing here ever blocks browsing.
    const updateBtn = document.getElementById('update-pill');
    let updateReady = false;

    function renderUpdate({ state, version, current, percent }) {
      updateReady = state === 'ready';
      updateBtn.hidden = !['available', 'downloading', 'ready', 'error'].includes(state);
      document.getElementById('app-version').textContent = current ? 'Oya Browser · v' + current : 'Oya Browser';
      updateBtn.classList.toggle('attention', state === 'available' || state === 'downloading' || updateReady);
      updateBtn.disabled = state === 'checking' || state === 'downloading';
      const v = current ? 'v' + current : '';
      if (state === 'checking')          updateBtn.textContent = 'Checking…';
      else if (state === 'available')    updateBtn.textContent = `Update ${version} available`;
      else if (state === 'downloading')  updateBtn.textContent = `Downloading ${version}… ${percent ?? 0}%`;
      else if (updateReady)              updateBtn.textContent = `Update ${version} ready — Restart`;
      else if (state === 'error')        updateBtn.textContent = `${v} — check failed`;
      else if (state === 'unsupported')  updateBtn.textContent = v;
      else                               updateBtn.textContent = `${v} — up to date`;
      updateBtn.title = updateReady ? 'Restart to finish updating'
        : state === 'unsupported' ? 'Updates apply to installed builds'
        : 'Click to check for updates';
    }

    oyaBrowser.getUpdateStatus().then(renderUpdate).catch(() => {});
    oyaBrowser.onUpdateStatus(renderUpdate);

    updateBtn.addEventListener('click', async () => {
      if (updateReady) { updateBtn.textContent = 'Restarting…'; oyaBrowser.installUpdate(); return; }
      renderUpdate({ state: 'checking' });
      renderUpdate(await oyaBrowser.checkForUpdates());
    });

    // ── Connection pill ──

    async function openReconnectDialog() {
      await oyaBrowser.showOverlay();
      try {
        const cfg = await oyaBrowser.getConfig();
        document.getElementById('reconn-server').value = cfg.serverUrl || '';
        document.getElementById('reconn-key').value = cfg.apiKey || '';
        document.getElementById('reconn-name').value = cfg.browserName || '';
        document.getElementById('reconn-error').textContent = '';
        document.getElementById('reconn-save').disabled = false;
        document.getElementById('reconn-save').textContent = 'Save & Reconnect';
        document.getElementById('reconnect-overlay').classList.add('open');
        window.activateDialog?.(document.getElementById('reconnect-overlay'), closeReconnectDialog);
        document.getElementById('reconn-server').focus();
      } catch (error) {
        oyaBrowser.hideOverlay();
        errorEl.textContent = error.message || 'Could not load connection settings.';
      }
    }

    function closeReconnectDialog() {
      clearTimeout(reconnectTimer);
      document.getElementById('reconnect-overlay').classList.remove('open');
      oyaBrowser.hideOverlay();
      window.deactivateDialog?.();
    }

    document.getElementById('connection-edit').addEventListener('click', () => { window.closeShellDialog(); openReconnectDialog(); });

    document.getElementById('reconn-cancel').addEventListener('click', closeReconnectDialog);

    let reconnectTimer;
    document.getElementById('reconn-save').addEventListener('click', async () => {
      const server = document.getElementById('reconn-server').value.trim();
      const key = document.getElementById('reconn-key').value.trim();
      const name = document.getElementById('reconn-name').value.trim();
      const message = document.getElementById('reconn-error');
      const button = document.getElementById('reconn-save');
      if (!/^wss?:\/\/[^\s]+$/.test(server) || !key) { message.textContent = 'Enter a ws:// or wss:// server address and an API key.'; return; }
      button.disabled = true; button.textContent = 'Connecting…'; message.textContent = '';
      clearTimeout(reconnectTimer);
      try {
        await oyaBrowser.saveConfig({ serverUrl: server, apiKey: key, browserName: name || undefined });
        document.getElementById('profile-server').textContent = server;
        document.getElementById('profile-title').textContent = name || 'This browser';
        reconnectTimer = setTimeout(() => {
          button.disabled = false; button.textContent = 'Retry connection';
          if (!connected) message.textContent = 'Could not connect. Check the server address and API key, then try again.';
        }, 5000);
      } catch (error) {
        message.textContent = error.message || 'Could not save connection settings.';
        button.disabled = false; button.textContent = 'Retry connection';
      }
    });

    document.getElementById('reconnect-overlay').addEventListener('click', e => {
      if (e.target.id === 'reconnect-overlay') closeReconnectDialog();
    });

    // ── Status updates ──

    function updatePill(isConnected, browserId) {
      connected = isConnected;
      document.getElementById('save-profile').disabled = !isConnected;
      if (isConnected) {
        connPill.className = 'conn-pill ok';
        connLabel.textContent = 'Connected';
        connPill.title = 'Connected' + (browserId ? ` — ${browserId}` : '');
      } else {
        connPill.className = 'conn-pill';
        connLabel.textContent = 'Offline';
        connPill.title = 'Not connected — click to configure';
      }
    }

    oyaBrowser.onWsStatus(s => {
      updatePill(s.connected, s.browserId);
      if (s.connected) {
        clearTimeout(reconnectTimer);
        document.getElementById('reconn-save').disabled = false;
        document.getElementById('reconn-save').textContent = 'Save & Reconnect';
        errorEl.textContent = '';
        document.getElementById('btn-connect').disabled = false;
        document.getElementById('btn-connect').textContent = 'Connect';
        // Close reconnect dialog if open
        if (document.getElementById('reconnect-overlay').classList.contains('open')) {
          closeReconnectDialog();
        }
      }
    });

    // Main process tells us to switch modes
    oyaBrowser.onModeChanged(mode => {
      body.className = mode === 'browsing' ? 'mode-browsing' : 'mode-setup';
    });

    // ── Fingerprint debug bar ──

    const fpContent = document.getElementById('fp-content');

    const saveProfileButton = document.getElementById('save-profile');
    const profileSaveStatus = document.getElementById('profile-save-status');
    let saveTimeout;
    oyaBrowser.onWsStatus(s => {
      fpContent.textContent = s.connected ? 'Profile · ' + (s.profileName || 'Default') + ' · Account sessions sync automatically' : 'Offline · Your account sessions remain on this desktop';
      saveProfileButton.disabled = !s.connected;
    });
    saveProfileButton.addEventListener('click', async () => {
      saveProfileButton.disabled = true;
      profileSaveStatus.textContent = 'Saving…';
      clearTimeout(saveTimeout);
      saveTimeout = setTimeout(() => { profileSaveStatus.textContent = 'Save not confirmed. Try again.'; saveProfileButton.disabled = false; }, 15000);
      try { await oyaBrowser.saveProfile(); }
      catch (error) { clearTimeout(saveTimeout); profileSaveStatus.textContent = error.message; saveProfileButton.disabled = false; }
    });
    oyaBrowser.onProfileSaved(state => {
      clearTimeout(saveTimeout);
      saveProfileButton.disabled = false;
      profileSaveStatus.textContent = state.error || ('Saved · ' + (state.sites?.length || 0) + ' sites');
    });

    // ── Init ──

    oyaBrowser.getConfig().then(cfg => {
      document.getElementById('cfg-server').value = cfg.serverUrl || '';
      document.getElementById('cfg-key').value = cfg.apiKey || '';
      document.getElementById('cfg-name').value = cfg.browserName || '';

      // If already configured, show setup briefly — ws-status will switch to browsing
      if (cfg.apiKey) {
        connPill.className = 'conn-pill trying';
        connLabel.textContent = 'Connecting\u2026';
      }
    });

    oyaBrowser.getStatus().then(s => {
      if (s.url) urlBar.value = s.url;
      updatePill(s.connected, s.browserId);
    });

    // ── Dev Panel ──

    const devBtn = document.getElementById('btn-dev');
    const devPanel = document.getElementById('dev-panel');
    const netLog = document.getElementById('net-log');
    let devOpen = false;
    let activeDevPane = 'record';
    let chatHistory = [];
    let chatSending = false;
    let netFilter = 'all';
    let cachedSource = { html: '', markdown: '' };

    function esc(s) {
      const d = document.createElement('div');
      d.textContent = s;
      return d.innerHTML;
    }

    // ── Dev panel resize ──

    (function() {
      const handle = document.getElementById('dev-panel-resize');
      let dragging = false, pendingWidth, resizeFrame;
      const flushResize = () => {
        resizeFrame = null;
        if (pendingWidth !== undefined) oyaBrowser.resizeDevPanel(pendingWidth);
        pendingWidth = undefined;
      };
      handle.addEventListener('pointerdown', e => {
        if (window.innerWidth < 960 || e.button !== 0) return;
        e.preventDefault();
        dragging = true;
        handle.setPointerCapture(e.pointerId);
        handle.classList.add('dragging');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
      });
      handle.addEventListener('pointermove', e => {
        if (!dragging) return;
        pendingWidth = Math.round(Math.max(320, Math.min(window.innerWidth - e.clientX, 560, window.innerWidth - 480)));
        if (!resizeFrame) resizeFrame = requestAnimationFrame(flushResize);
      });
      const finishResize = () => {
        if (!dragging) return;
        dragging = false;
        cancelAnimationFrame(resizeFrame);
        flushResize();
        handle.classList.remove('dragging');
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      };
      handle.addEventListener('pointerup', finishResize);
      handle.addEventListener('pointercancel', finishResize);
      handle.addEventListener('lostpointercapture', finishResize);
      window.addEventListener('blur', finishResize);
    })();

    // ── Tab switching ──

    document.querySelectorAll('.dev-tab').forEach(tab => {
      tab.addEventListener('click', () => {
        if (tab.dataset.pane) showDevPane(tab.dataset.pane);
      });
    });

    devBtn.addEventListener('click', () => {
      oyaBrowser.toggleDevPanel();
    });

    oyaBrowser.onDevPanelState(open => {
      devOpen = open;
      devPanel.inert = !open;
      devBtn.classList.toggle('active', open);
    });

    document.getElementById('dev-clear').addEventListener('click', () => {
      if (activeDevPane === 'chat') {
        chatHistory = [];
        const el = document.getElementById('chat-messages');
        el.innerHTML = '<div class="chat-empty">Ask me to do something in the browser...</div>';
      }
      else if (activeDevPane === 'network') netLog.innerHTML = '';
      else if (activeDevPane === 'source') {
        cachedSource = { html: '', markdown: '' };
        document.getElementById('source-markdown').textContent = 'Right-click → View Page Source or click Refresh';
        document.getElementById('source-markdown').classList.add('empty');
        document.getElementById('source-html').textContent = 'Right-click → View Page Source or click Refresh';
        document.getElementById('source-html').classList.add('empty');
      }
      else if (activeDevPane === 'actions') {
        const r = document.getElementById('action-result');
        r.textContent = '';
        r.classList.remove('visible', 'error');
      }
    });

    // ── Log entry helper ──

    function fmtTime(ts) {
      const d = new Date(ts);
      return [d.getHours(), d.getMinutes(), d.getSeconds()]
        .map(n => String(n).padStart(2, '0')).join(':') + '.' + String(d.getMilliseconds()).padStart(3, '0');
    }

    function createLogEntry(entry) {
      const div = document.createElement('div');
      div.className = 'dev-entry';
      div.dataset.dir = entry.dir;
      div.dataset.type = entry.type;

      div.innerHTML = `<div class="head">
        <span class="ts">${fmtTime(entry.ts)}</span>
        <span class="dir ${entry.dir}">${entry.dir === 'in' ? '\u25BC IN' : '\u25B2 OUT'}</span>
        <span class="msg-type">${esc(entry.type)}</span>
      </div>
      <div class="body">${esc(entry.data || '')}</div>`;

      div.addEventListener('click', () => div.classList.toggle('expanded'));
      return div;
    }

    // ── Dev log events ──

    oyaBrowser.onDevLog(entry => {
      // Add to network pane (with filtering)
      const netEntry = createLogEntry(entry);
      netEntry.style.display = shouldShowNetEntry(entry) ? '' : 'none';
      netLog.appendChild(netEntry);
      if (netLog.scrollHeight - netLog.scrollTop - netLog.clientHeight < 60) {
        netLog.scrollTop = netLog.scrollHeight;
      }
      while (netLog.children.length > 500) netLog.removeChild(netLog.firstChild);
    });

    // ── Network filters ──

    function shouldShowNetEntry(entry) {
      if (netFilter === 'all') return true;
      if (netFilter === 'in') return entry.dir === 'in';
      if (netFilter === 'out') return entry.dir === 'out';
      if (netFilter === 'cmd') return entry.type.startsWith('cmd:') || entry.type === 'auth';
      if (netFilter === 'result') return entry.type.startsWith('result:');
      return true;
    }

    document.querySelectorAll('.net-filter').forEach(btn => {
      btn.addEventListener('click', () => {
        netFilter = btn.dataset.filter;
        document.querySelectorAll('.net-filter').forEach(b => b.classList.toggle('active', b.dataset.filter === netFilter));
        // Re-filter existing entries
        netLog.querySelectorAll('.dev-entry').forEach(entry => {
          const dir = entry.dataset.dir;
          const type = entry.dataset.type;
          const show = netFilter === 'all'
            || (netFilter === 'in' && dir === 'in')
            || (netFilter === 'out' && dir === 'out')
            || (netFilter === 'cmd' && (type.startsWith('cmd:') || type === 'auth'))
            || (netFilter === 'result' && type.startsWith('result:'));
          entry.style.display = show ? '' : 'none';
        });
      });
    });

    // ── Actions ──

    function getActionParams(btn) {
      const from = btn.dataset.from;
      if (!from) return {};
      const ids = from.split(',');
      const action = btn.dataset.action;
      // Map param inputs to action params
      if (action === 'navigate') return { url: document.getElementById(ids[0]).value.trim() };
      if (action === 'click') return { element_id: document.getElementById(ids[0]).value.trim() };
      if (action === 'type') return { element_id: document.getElementById(ids[0]).value.trim(), text: document.getElementById(ids[1]).value };
      if (action === 'press-key') return { key: document.getElementById(ids[0]).value.trim() };
      if (action === 'hover') return { element_id: document.getElementById(ids[0]).value.trim() };
      if (action === 'click-coords') return { x: Number(document.getElementById(ids[0]).value), y: Number(document.getElementById(ids[1]).value) };
      if (action === 'wait') return { selector: document.getElementById(ids[0]).value.trim() };
      if (action === 'select') return { element_id: document.getElementById(ids[0]).value.trim(), value: document.getElementById(ids[1]).value };
      return {};
    }

    document.querySelectorAll('.action-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const action = btn.dataset.action;
        if (!action || btn.classList.contains('running')) return;
        btn.classList.add('running');
        const resultEl = document.getElementById('action-result');

        try {
          const params = getActionParams(btn);
          const result = await oyaBrowser.devAction(action, params);
          resultEl.classList.remove('error');
          resultEl.classList.add('visible');

          if (action === 'screenshot' && result?.ok && result.data?.screenshot) {
            resultEl.innerHTML = '<img style="width:100%;border-radius:4px;" src="' + esc(result.data.screenshot) + '">';
          } else if (action === 'analyze' && result?.ok && result.data?.markdown) {
            resultEl.textContent = result.data.markdown.slice(0, 5000);
          } else if (result?.ok) {
            resultEl.textContent = JSON.stringify(result.data || result, null, 2).slice(0, 3000);
          } else {
            resultEl.textContent = result?.error || 'Unknown error';
            resultEl.classList.add('error');
          }
        } catch (e) {
          resultEl.textContent = e.message;
          resultEl.classList.add('error', 'visible');
        }
        btn.classList.remove('running');
      });
    });

    // ── Chat ──

    const chatMessagesEl = document.getElementById('chat-messages');
    const chatInputEl = document.getElementById('chat-input');
    const chatSendBtn = document.getElementById('chat-send');

    function mdToHtml(text) {
      let s = esc(text);
      // Code blocks: ```...```
      s = s.replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>');
      // Inline code: `...`
      s = s.replace(/`([^`]+)`/g, '<code>$1</code>');
      // Bold: **...**
      s = s.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
      // Italic: *...*
      s = s.replace(/(?<!\*)\*([^*]+)\*(?!\*)/g, '<em>$1</em>');
      // Headings: ### ... (at line start)
      s = s.replace(/^#{3,}\s+(.+)$/gm, '<strong style="color:var(--text)">$1</strong>');
      s = s.replace(/^##\s+(.+)$/gm, '<strong style="color:var(--text);font-size:13px">$1</strong>');
      s = s.replace(/^#\s+(.+)$/gm, '<strong style="color:var(--text);font-size:14px">$1</strong>');
      // Bullet lists: - ... or * ...
      s = s.replace(/^[\-\*]\s+(.+)$/gm, '\u2022 $1');
      // Numbered lists: 1. ...
      s = s.replace(/^\d+\.\s+(.+)$/gm, function(m, p1) { return '\u2022 ' + p1; });
      // Line breaks
      s = s.replace(/\n/g, '<br>');
      return s;
    }

    function addChatMessage(role, content, toolCalls) {
      // Remove empty state
      const empty = chatMessagesEl.querySelector('.chat-empty');
      if (empty) empty.remove();
      // Remove thinking indicator
      const thinking = chatMessagesEl.querySelector('.chat-thinking');
      if (thinking) thinking.remove();

      const div = document.createElement('div');
      div.className = 'chat-msg ' + role;
      if (role === 'assistant' && content.startsWith('Error:')) div.classList.add('error');

      let html = role === 'user' ? esc(content) : mdToHtml(content);
      if (toolCalls && toolCalls.length > 0) {
        html += '<div class="chat-tools">';
        toolCalls.forEach(t => {
          html += '<span class="chat-tool-badge">' + esc(t.name) + '</span>';
        });
        html += '</div>';
      }
      div.innerHTML = html;
      if (role === 'assistant' && !content.startsWith('Error:')) {
        const copyBtn = document.createElement('button');
        copyBtn.className = 'chat-copy';
        copyBtn.textContent = 'Copy';
        copyBtn.addEventListener('click', (e) => {
          e.stopPropagation();
          navigator.clipboard.writeText(content).then(() => {
            copyBtn.textContent = 'Copied!';
            setTimeout(() => { copyBtn.textContent = 'Copy'; }, 1500);
          });
        });
        div.appendChild(copyBtn);
      }
      chatMessagesEl.appendChild(div);
      chatMessagesEl.scrollTop = chatMessagesEl.scrollHeight;
    }

    function showThinking() {
      const empty = chatMessagesEl.querySelector('.chat-empty');
      if (empty) empty.remove();
      const div = document.createElement('div');
      div.className = 'chat-thinking';
      div.innerHTML = '<div class="chat-dots"><span></span><span></span><span></span></div> Thinking...';
      chatMessagesEl.appendChild(div);
      chatMessagesEl.scrollTop = chatMessagesEl.scrollHeight;
    }

    async function sendChat() {
      const text = chatInputEl.value.trim();
      if (!text || chatSending) return;

      chatSending = true;
      chatSendBtn.disabled = true;
      chatInputEl.value = '';

      const userMsg = { role: 'user', content: text };
      chatHistory.push(userMsg);
      addChatMessage('user', text);
      showThinking();

      try {
        const messages = chatHistory.map(m => ({ role: m.role, content: m.content }));
        const data = await oyaBrowser.sendChat(messages);

        if (data.error) {
          const errMsg = { role: 'assistant', content: 'Error: ' + data.error };
          chatHistory.push(errMsg);
          addChatMessage('assistant', errMsg.content);
        } else {
          const assistantMsg = { role: 'assistant', content: data.text || '(no response)', toolCalls: data.toolCalls || [] };
          chatHistory.push(assistantMsg);
          addChatMessage('assistant', assistantMsg.content, assistantMsg.toolCalls);
        }
      } catch (e) {
        const errMsg = { role: 'assistant', content: 'Error: ' + e.message };
        chatHistory.push(errMsg);
        addChatMessage('assistant', errMsg.content);
      }

      chatSending = false;
      chatSendBtn.disabled = false;
      chatInputEl.focus();
    }

    chatSendBtn.addEventListener('click', sendChat);
    chatInputEl.addEventListener('keydown', e => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
        e.preventDefault();
        sendChat();
      }
    });

    // ── Source ──

    const srcMarkdownEl = document.getElementById('source-markdown');
    const srcHtmlEl = document.getElementById('source-html');

    document.getElementById('source-refresh').addEventListener('click', async () => {
      const btn = document.getElementById('source-refresh');
      btn.textContent = '⟳ Loading...';
      try {
        cachedSource = await oyaBrowser.getPageSource();
        renderSource();
      } catch (e) {
        srcMarkdownEl.textContent = 'Error: ' + e.message;
        srcMarkdownEl.classList.remove('empty');
      }
      btn.textContent = '⟳ Refresh';
    });

    function renderSource() {
      if (cachedSource.markdown) {
        srcMarkdownEl.textContent = cachedSource.markdown;
        srcMarkdownEl.classList.remove('empty');
      } else {
        srcMarkdownEl.textContent = 'No markdown available';
        srcMarkdownEl.classList.add('empty');
      }
      if (cachedSource.html) {
        srcHtmlEl.textContent = cachedSource.html;
        srcHtmlEl.classList.remove('empty');
      } else {
        srcHtmlEl.textContent = 'No HTML available';
        srcHtmlEl.classList.add('empty');
      }
    }

    function showDevPane(pane) {
      document.querySelectorAll('.dev-tab').forEach(t => t.classList.toggle('active', t.dataset.pane === pane));
      document.querySelectorAll('.dev-pane').forEach(p => p.classList.toggle('active', p.id === 'pane-' + pane));
      activeDevPane = pane;
      window.syncToolNavigation?.(pane);
      oyaBrowser.saveUiPreferences({ pane });
    }

    const switchToSourcePane = () => showDevPane('source');

    // ── View Page Source ──

    oyaBrowser.onViewSource((data) => {
      switchToSourcePane();
      cachedSource = { html: data.html || '', markdown: data.markdown || '' };
      renderSource();
    });

    // ── Inspect Element Result ──

    oyaBrowser.onInspectResult((result) => {
      switchToSourcePane();
      if (result?.ok && result.data?.markdown) {
        cachedSource = { html: cachedSource?.html || '', markdown: result.data.markdown };
      } else {
        cachedSource = { html: cachedSource?.html || '', markdown: 'Error: ' + (result?.error || 'Unknown error') };
      }
      renderSource();
    });
