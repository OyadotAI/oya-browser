/**
 * State the shell's scripts share: whether the browser is connected, whether
 * the workspace panel is open, and which pane it shows.
 */
/* exported ShellState */

/** Shared, mutable shell state. */
const ShellState = {
  /** Connected to the server (set by the connection status). */
  connected: false,
  /** The workspace panel is open (set by the dev panel). */
  devOpen: false,
  /** The pane the workspace panel shows. */
  activeDevPane: 'chat',
  /** The Inspect sub-pane last shown, which the Inspect tab reopens. */
  lastInspect: 'actions',
  /** A workflow is being recorded (set by the studio view). */
  recording: false,
};
