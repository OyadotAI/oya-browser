# Product design

1. Inspect the rendered surface and the current component code. Inventory existing tokens,
   typography, controls, interaction patterns, and brand assets. Read the project's frontend
   skill when available. Reuse working patterns before adding another visual language.
2. Describe the task flow, hierarchy, primary action, and recovery path. For each changed region,
   map empty, loading, partial, success, denied, failed, disconnected, and long-content states.
   Show what action remains available in each; avoid badges that imply an unavailable fix.
3. Specify concrete layout, spacing, labels, truncation, overflow, and responsive behavior. Check
   the smallest supported app window as well as a comfortable desktop size. Dense tools need
   readable rows, quiet metadata, and controls that retain their labels under pressure.
4. Check keyboard order, visible focus, escape/dismiss behavior, accessible names, semantic
   controls, contrast, reduced motion, and light/dark themes. Trace focus after dialogs and
   asynchronous updates. Do not use color as the only state signal.
5. Inspect actual screenshots through Oya for browser surfaces. A browser fixture of native
   components cannot establish native integration correctness. Identify the evidence type.
6. Hand off `../assets/design.md` with implementable values and state transitions, and identify
   existing image/icon assets to reuse. Generate or request missing imagery only when it serves
   the task; never disguise a placeholder as a finished brand asset.

Return a design or findings. Change implementation only when explicitly assigned that work.
