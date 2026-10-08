/** Keyboard discovery is read from the connected desktop, never guessed from the server OS. */

/** The agent loop and MCP share the same read-only tool definition. */
export const KEYBOARD_TOOLS = [
  {
    type: 'function',
    function: {
      name: 'list_keyboard_shortcuts',
      description:
        'Learn the connected Oya Browser’s keyboard shortcuts. Returns its platform and live shell bindings with command, description, category, displayed chord, key and physical code. Read-only: does not press keys or change focus. Use this to explain shortcuts to the user; prefer dedicated agent tools for actions. press_key targets webpage input, not shell shortcut execution.',
      parameters: { type: 'object', properties: {}, required: [], additionalProperties: false },
    },
  },
];
