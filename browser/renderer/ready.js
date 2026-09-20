/**
 * Loaded last, once every shell module has run: publishes the shell's icon
 * helper on window. Its presence is the signal that the renderer finished
 * loading, which the desktop tests (tests/integration/shell.mjs) wait on.
 */
/* global ShellIcons */

window.shellIcon = (name) => ShellIcons.icon(name);
