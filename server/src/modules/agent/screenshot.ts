/**
 * Screenshots the model can see. A tool result is text only in the OpenAI chat
 * shape, so the picture follows the result as a user turn with an image part.
 * Only the latest screenshot is kept: each one costs as much as a long page
 * of text, and an old one shows a page that is gone.
 */
import { sendCommand } from '../browsers/socket.ts';

/** Said instead of a screenshot when the task holds secrets: the picture cannot be redacted. */
const WITHHELD =
  'Screenshot withheld: this task has secret values that could be visible on the page. Use analyze_page instead.';
/** What an earlier screenshot turn becomes once a newer one arrives. */
const REPLACED = '(An earlier screenshot, removed: the page has changed since.)';
/** The text that goes with the image. */
const CAPTION = 'Screenshot of the visible page, as a person would see it:';

/** A screenshot tool call's result: the text for the model, and the image when there is one. */
export type Screenshot = {
  /** The tool result the model reads. */
  text: string;
  /** The image as a data URL. */
  image?: string;
};

/** Captures the visible page as a JPEG, unless the run holds secrets a screenshot could show. */
export async function takeScreenshot(browserId: string, secrets: Record<string, any>): Promise<Screenshot> {
  if (Object.keys(secrets || {}).length) return { text: WITHHELD };
  const r = await sendCommand(browserId, 'screenshot', { format: 'jpeg' });
  if (!r.ok) return { text: `Error: ${r.error}` };
  if (!r.data?.screenshot) return { text: 'Screenshot captured but no image data returned' };
  return { text: 'Screenshot captured; it follows as an image.', image: r.data.screenshot };
}

/** Whether a message is a screenshot turn made by addImageTurn. */
const isImageTurn = (m) =>
  m.role === 'user' && Array.isArray(m.content) && m.content.some((p) => p.type === 'image_url');

/** Adds the screenshot turn, replacing any earlier one with a short note. */
export function addImageTurn(messages: any[], image: string) {
  for (const m of messages) if (isImageTurn(m)) m.content = REPLACED;
  messages.push({
    role: 'user',
    content: [
      { type: 'text', text: CAPTION },
      { type: 'image_url', image_url: { url: image } },
    ],
  });
}
