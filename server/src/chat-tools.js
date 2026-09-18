/**
 * Browser tool definitions for the chat LLM — maps to MCP tools / sendCommand.
 */

export const BROWSER_TOOLS = [
  {
    type: 'function',
    function: {
      name: 'analyze_page',
      description: `Analyze the current page. Returns structured markdown with all interactive elements numbered as [#id type "label"].
Use element IDs with click/type tools. Includes page metadata, full content, and element index with visibility flags.`,
      parameters: { type: 'object', properties: {}, additionalProperties: false },
    },
  },
  {
    type: 'function',
    function: {
      name: 'navigate',
      description: 'Navigate the browser to a URL.',
      parameters: {
        type: 'object',
        properties: { url: { type: 'string', description: 'The URL to navigate to' } },
        required: ['url'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'click',
      description: 'Click an interactive element by its ID number (from analyze_page results).',
      parameters: {
        type: 'object',
        properties: { element_id: { type: 'number', description: 'The element ID from analyze_page' } },
        required: ['element_id'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'press_key',
      description: 'Press a safe navigation key. Allowed: Enter, Escape, Tab, ArrowDown, ArrowUp, ArrowLeft, ArrowRight, Backspace, Delete, Space, Home, End, PageUp, PageDown. Do NOT press F-keys, Meta, Control, Alt, or Shift.',
      parameters: {
        type: 'object',
        properties: { key: { type: 'string', enum: ['Enter', 'Escape', 'Tab', 'ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Backspace', 'Delete', 'Space', 'Home', 'End', 'PageUp', 'PageDown'], description: 'Key to press' } },
        required: ['key'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'type',
      description: 'Type text into an input element by its ID (from analyze_page). Clears existing content first.',
      parameters: {
        type: 'object',
        properties: {
          element_id: { type: 'number', description: 'The element ID from analyze_page' },
          text: { type: 'string', description: 'The text to type' },
        },
        required: ['element_id', 'text'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'select_option',
      description: 'Choose an option in a native dropdown (a select element from analyze_page) by the option text you see. Accepts {{placeholders}} and filters.',
      parameters: {
        type: 'object',
        properties: {
          element_id: { type: 'number', description: 'The select element ID from analyze_page' },
          option: { type: 'string', description: 'The visible text of the option to choose' },
        },
        required: ['element_id', 'option'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'upload_file',
      description: `Attach one of the task's files to an upload field. Only the files listed under FILES exist; you cannot upload anything else, and you never see or need the contents.
The real <input type="file"> is normally hidden behind a styled "Choose file" / "Upload" / "Attach" button or a drop zone, so it often has no element id of its own: pass the id of the visible button, drop zone or field and the input behind it is found. Omit element_id only when the page has a single upload field.`,
      parameters: {
        type: 'object',
        properties: {
          name: { type: 'string', description: 'The file\'s name in the task data, as listed under FILES (not the filename)' },
          element_id: { type: 'number', description: 'The upload button, drop zone or file field from analyze_page' },
        },
        required: ['name'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'screenshot',
      description: 'Capture a screenshot of the visible browser tab.',
      parameters: { type: 'object', properties: {}, additionalProperties: false },
    },
  },
  {
    type: 'function',
    function: {
      name: 'scroll',
      description: 'Scroll the page up or down.',
      parameters: {
        type: 'object',
        properties: {
          direction: { type: 'string', enum: ['up', 'down'], description: 'Scroll direction' },
          amount: { type: 'number', description: 'Pixels to scroll (default 500)' },
        },
        required: ['direction'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'wait',
      description: 'Wait for an element matching a CSS selector to appear.',
      parameters: {
        type: 'object',
        properties: {
          selector: { type: 'string', description: 'CSS selector to wait for' },
          timeout: { type: 'number', description: 'Max wait time in ms (default 10000)' },
        },
        required: ['selector'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'read_elements',
      description: 'List interactive elements on the page. Lighter than analyze_page.',
      parameters: {
        type: 'object',
        properties: {
          selector: { type: 'string', description: 'Optional CSS selector to scope search' },
          limit: { type: 'number', description: 'Max elements to return (default 50)' },
        },
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'list_tabs',
      description: 'List all open tabs (ID, title, URL, which is active).',
      parameters: { type: 'object', properties: {}, additionalProperties: false },
    },
  },
  {
    type: 'function',
    function: {
      name: 'open_tab',
      description: 'Open a new browser tab, optionally navigating to a URL.',
      parameters: {
        type: 'object',
        properties: { url: { type: 'string', description: 'URL to open (optional)' } },
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'switch_tab',
      description: 'Switch to a different tab by ID (from list_tabs).',
      parameters: {
        type: 'object',
        properties: { tab_id: { type: 'number', description: 'The tab ID to switch to' } },
        required: ['tab_id'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'handle_dialog',
      description: 'Answer a native browser dialog (confirm or prompt) that is blocking the page. Alerts are answered for you. Accept only what the task actually asks for — a confirm may be guarding something destructive.',
      parameters: {
        type: 'object',
        properties: {
          accept: { type: 'boolean', description: 'true clicks OK, false clicks Cancel' },
          prompt_text: { type: 'string', description: 'The text to enter, for a prompt dialog only' },
        },
        required: ['accept'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'close_tab',
      description: 'Close a tab. Closes active tab if no tab_id specified.',
      parameters: {
        type: 'object',
        properties: { tab_id: { type: 'number', description: 'Tab ID to close (optional)' } },
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'click_coordinates',
      description: 'Click at specific x,y pixel coordinates on the page.',
      parameters: {
        type: 'object',
        properties: {
          x: { type: 'number', description: 'X coordinate in pixels' },
          y: { type: 'number', description: 'Y coordinate in pixels' },
        },
        required: ['x', 'y'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'mouse_move',
      description: 'Move mouse to x,y coordinates without clicking. Triggers hover states and tooltips.',
      parameters: {
        type: 'object',
        properties: {
          x: { type: 'number', description: 'X coordinate in pixels' },
          y: { type: 'number', description: 'Y coordinate in pixels' },
        },
        required: ['x', 'y'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'double_click',
      description: 'Double-click an element by ID or at x,y coordinates.',
      parameters: {
        type: 'object',
        properties: {
          element_id: { type: 'number', description: 'Element ID (from analyze_page)' },
          x: { type: 'number', description: 'X coordinate (if no element_id)' },
          y: { type: 'number', description: 'Y coordinate (if no element_id)' },
        },
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'keyboard_type',
      description: 'Type text into whatever is currently focused, without targeting a specific element.',
      parameters: {
        type: 'object',
        properties: {
          text: { type: 'string', description: 'Text to type' },
        },
        required: ['text'],
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'drag',
      description: 'Drag from one point to another. For sliders, drag-and-drop, or text selection.',
      parameters: {
        type: 'object',
        properties: {
          from_x: { type: 'number', description: 'Start X' },
          from_y: { type: 'number', description: 'Start Y' },
          to_x: { type: 'number', description: 'End X' },
          to_y: { type: 'number', description: 'End Y' },
        },
        required: ['from_x', 'from_y', 'to_x', 'to_y'],
        additionalProperties: false,
      },
    },
  },
];
