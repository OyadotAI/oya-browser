/**
 * Browser tool definitions for the chat LLM, maps to MCP tools / sendCommand.
 */
import { RUN_SCRIPT_OUTPUT_CHARS, WAIT_FOR_DEFAULT_MS, WAIT_FOR_MAX_MS } from './constants.ts';

/** A tool with no arguments. */
export const bare = (name: string, description: string) => ({
  type: 'function',
  function: { name, description, parameters: { type: 'object', properties: {}, additionalProperties: false } },
});

/** A tool that takes one argument, required. */
const single = (name: string, description: string, arg: string, schema: Record<string, any>) => ({
  type: 'function',
  function: {
    name,
    description,
    parameters: { type: 'object', properties: { [arg]: schema }, required: [arg], additionalProperties: false },
  },
});

/** The page tools of page-tool-handlers.ts: reading with code, waiting, finding, hovering and history. */
const PAGE_TOOLS = [
  single(
    'run_script',
    `Run JavaScript that READS the page and returns data, for what analyze_page does not show well: every row of a long table, attributes, counts, text across many elements. The body of an async function: \`return\` the value (JSON-serialisable, cut at ${RUN_SCRIPT_OUTPUT_CHARS} chars). It runs where the page cannot see it and must not change anything: no clicks, typing, requests or navigation; act with the element tools, which replay. Example: return [...document.querySelectorAll('table tr')].map(r => r.innerText);`,
    'script',
    { type: 'string', description: 'The function body; `return` what you want to read' },
  ),
  {
    type: 'function',
    function: {
      name: 'wait_for',
      description:
        'Wait until the page is ready: some text shows, the url contains something, and/or the network has gone quiet. With no text or url it waits for the network to settle. Use it after an action that loads results in the background, rather than analyzing again and again.',
      parameters: {
        type: 'object',
        properties: {
          text: { type: 'string', description: 'Text that must appear on the page' },
          url: { type: 'string', description: 'Part of the url the page must reach' },
          network_idle: { type: 'boolean', description: 'Also wait for no new requests for a moment' },
          timeout: {
            type: 'number',
            description: `Max wait in ms (default ${WAIT_FOR_DEFAULT_MS}, at most ${WAIT_FOR_MAX_MS})`,
          },
        },
        additionalProperties: false,
      },
    },
  },
  single(
    'find',
    'Find the elements that match a description ("search box", "add to cart", "next page"), best first, with ids click and type accept. Much shorter than analyze_page when you know what you are looking for.',
    'query',
    { type: 'string', description: 'The words the element is known by' },
  ),
  single(
    'hover',
    'Move the mouse onto an element, for menus, tooltips and previews that open on hover. Tells you what appeared.',
    'element_id',
    { type: 'number', description: 'Element ID from analyze_page' },
  ),
  bare('go_back', "Go back one page in the tab's history, like the browser's back button, and wait for it to load."),
  bare('go_forward', "Go forward one page in the tab's history and wait for it to load."),
  bare('reload', 'Reload the page and wait for it to load. For a page stuck loading or showing stale data.'),
];

/** OpenAI-style function tool schemas offered to the chat model. */
export const BROWSER_TOOLS = [
  {
    type: 'function',
    function: {
      name: 'analyze_page',
      description: `Analyze the current page: its facts (url, title, scroll), everything on it in reading order, and every element you can act on, each with an id for the click and type tools.
Pass content false while you are working the page rather than reading it: you get the elements alone, which is far shorter and enough to click and type.`,
      parameters: {
        type: 'object',
        properties: {
          content: {
            type: 'boolean',
            description: "Include the page's words (default true). False returns only the elements to act on.",
          },
          selector: {
            type: 'string',
            description:
              'CSS selector of one part of the page (a dialog, a results table) to analyze alone, for a long page where only that part matters.',
          },
        },
        additionalProperties: false,
      },
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
      description:
        'Press a safe navigation key. Allowed: Enter, Escape, Tab, ArrowDown, ArrowUp, ArrowLeft, ArrowRight, Backspace, Delete, Space, Home, End, PageUp, PageDown. Do NOT press F-keys, Meta, Control, Alt, or Shift.',
      parameters: {
        type: 'object',
        properties: {
          key: {
            type: 'string',
            enum: [
              'Enter',
              'Escape',
              'Tab',
              'ArrowDown',
              'ArrowUp',
              'ArrowLeft',
              'ArrowRight',
              'Backspace',
              'Delete',
              'Space',
              'Home',
              'End',
              'PageUp',
              'PageDown',
            ],
            description: 'Key to press',
          },
        },
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
      description:
        'Choose an option in a native dropdown (a select element from analyze_page) by the option text you see. Accepts {{placeholders}} and filters.',
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
          name: {
            type: 'string',
            description: "The file's name in the task data, as listed under FILES (not the filename)",
          },
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
      description:
        'See the visible browser tab as an image. Use it when layout, images, icons or a canvas matter, or when analyze_page does not explain what is on screen. Element ids still come from analyze_page.',
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
      description:
        'List interactive elements on the page, with the same ids analyze_page gives. Lighter than analyze_page: no page text.',
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
      name: 'read_console',
      description:
        'Read what the page logged: its own errors and warnings. Use this when a step failed, a page stalled, or the portal showed an error you cannot read on screen, it says what the page itself complained about.',
      parameters: {
        type: 'object',
        properties: {
          level: { type: 'string', description: "Only this level: 'error', 'warning', 'info' or 'debug' (optional)" },
          pattern: { type: 'string', description: 'Only messages matching this regular expression (optional)' },
          limit: { type: 'number', description: 'How many entries, newest first (default 100)' },
        },
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'read_network',
      description:
        'Read the requests the page made and how the server answered. Use failed_only when a submit or a handoff did not work: a 400 or 500 here names the request the server refused, which is the difference between bad data and a broken page. Judge only requests to the site you are working on, adverts, trackers and analytics fail on almost every commercial page and explain nothing about your task.',
      parameters: {
        type: 'object',
        properties: {
          failed_only: { type: 'boolean', description: 'Only requests that failed or answered 400 and above' },
          pattern: { type: 'string', description: 'Only urls matching this regular expression (optional)' },
          limit: { type: 'number', description: 'How many requests, newest first (default 100)' },
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
      description:
        'Answer a native browser dialog (confirm or prompt) that is blocking the page. Alerts are answered for you. Accept only what the task actually asks for, a confirm may be guarding something destructive.',
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
  ...PAGE_TOOLS,
];

/**
 * The browser command a tool needs, for the tools not every browser does: a CDP
 * browser has no console or network log, and an older desktop app no scripts or
 * history moves. Offering a tool the browser refuses only costs the model a turn.
 */
const NEEDS: Record<string, string> = {
  read_console: 'read_console',
  read_network: 'read_network',
  // run_script is server-internal, so no browser announces it; the app that first
  // ran scripts is the one that first did history moves, so reload stands for both.
  run_script: 'reload',
  wait_for: 'reload',
  hover: 'hover',
  go_back: 'back',
  go_forward: 'forward',
  reload: 'reload',
};

/** The browser tools a browser that does `actions` can run; all of them when its actions are not known. */
export const toolsOn = (actions: readonly string[] | null) =>
  actions
    ? BROWSER_TOOLS.filter((t) => !Object.hasOwn(NEEDS, t.function.name) || actions.includes(NEEDS[t.function.name]))
    : BROWSER_TOOLS;
