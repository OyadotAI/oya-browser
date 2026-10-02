/**
 * Loaded last, once every shell module has run: turns every orb placeholder
 * into an orb, plays the launch, then publishes the shell's icon helper on window. Its presence is the signal that the renderer finished
 * loading, which the desktop tests (tests/integration/shell.mjs) wait on.
 */
/* global ShellIcons, OyaOrb, Launch */

OyaOrb.mountAll();
OyaOrb.followVisibility();
Launch.play();
window.shellIcon = (name) => ShellIcons.icon(name);
