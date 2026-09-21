/** POST /control/sessions/:id/input: a person's input, sent to the browser under their takeover. */
import { hash, fault } from '../service.ts';
import { sendCommand } from '../../browsers/socket.ts';
import * as flow from '../../playbooks/flow-recorder.ts';
import { Status } from '../../../platform/http-status.ts';

/** The browser actions a person may send. */
const HUMAN_INPUTS = [
  'click',
  'type',
  'press_key',
  'scroll',
  'click_coordinates',
  'double_click',
  'drag',
  'mouse_move',
  'scroll_at',
  'type_text',
  'keyboard_type',
  'navigate',
  'back',
  'forward',
  'reload',
  'screenshot',
  'analyze',
  'read_page',
];

/** Sends the input as the person's command and returns the browser's result. */
export async function humanInput(req) {
  if (!HUMAN_INPUTS.includes(req.body?.action))
    throw fault('invalid_action', 'Unsupported human input', Status.BAD_REQUEST);
  const params = req.body.params || {};
  const result = await sendCommand(req.params.id, req.body.action, params, undefined, hash(req.authToken));
  // A recording in progress keeps the navigations the person asked for in the live
  // view; what they click and type is seen in the page itself.
  if (result?.ok !== false) flow.noteCommand(req.params.id, req.body.action, req.body.params || {});
  return result;
}
