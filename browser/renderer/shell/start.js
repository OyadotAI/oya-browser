/**
 * The Oya start page: where a new tab opens. It shows while the active tab is on
 * the start page (the main process reports it in the tab list), and a task typed
 * there goes to the agent through the Ask pane, which already handles sign-in,
 * the model key and every answer, so the start page never duplicates any of it.
 */
/* global oyaBrowser, Dom, ShellState, DevPanel, Chat */
/* exported StartPage */

/** The start page. */
const StartPage = {
  /** The tab list changed: the start page shows exactly while the active tab is on it. */
  update(tabs) {
    const home = !!tabs.find((tab) => tab.active)?.home;
    const page = Dom.byId('start-page');
    if (page.hidden === !home) return;
    page.hidden = !home;
    document.body.classList.toggle('on-home', home);
    if (home) StartPage.arrive();
  },

  /** Plays the page's arrival again and puts the cursor in the task box. */
  arrive() {
    const page = Dom.byId('start-page');
    page.classList.remove('arriving');
    void page.offsetWidth;
    page.classList.add('arriving');
    Dom.byId('start-input').focus();
  },

  /** Hands a task to the agent: the Ask pane opens with it and sends it. */
  async ask(text) {
    const task = String(text || '').trim();
    if (!task || Chat.sending) return;
    if (!ShellState.devOpen) await oyaBrowser.toggleDevPanel();
    DevPanel.show('chat');
    Dom.byId('chat-input').value = task;
    Dom.byId('start-input').value = '';
    await Chat.send();
  },

  /** The task box was submitted. */
  submit(event) {
    event.preventDefault();
    StartPage.ask(Dom.byId('start-input').value);
  },

  /** Enter starts the task; Shift+Enter is a new line. */
  keydown(event) {
    if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
    StartPage.submit(event);
  },

  /** An example task was picked: it runs as if typed. */
  example(event) {
    const task = event.target.closest('[data-task]')?.dataset.task;
    if (task) StartPage.ask(task);
  },
};

oyaBrowser.onTabsUpdated(StartPage.update);
Dom.byId('start-ask').addEventListener('submit', StartPage.submit);
Dom.byId('start-input').addEventListener('keydown', StartPage.keydown);
Dom.byId('start-examples').addEventListener('click', StartPage.example);
