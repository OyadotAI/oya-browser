/**
 * Every number and fixed string Sign in with ChatGPT runs on, by name: OpenAI's
 * addresses, what sign-in asks for, and the limits on each step.
 */

/** Where the browser signs in. */
export const AUTHORIZE_URL = 'https://auth.openai.com/api/accounts/authorize';
/** Where codes and refresh tokens are exchanged. */
export const TOKEN_URL = 'https://auth.openai.com/api/accounts/oauth/token';
/** The API the plan's tokens are for. */
export const RESOURCE = 'https://api.openai.com/v1';
/** The one address a model call ever goes to from this app; nothing the server sends can change it. */
export const RESPONSES_URL = 'https://api.openai.com/v1/responses';
/** What sign-in asks for; chatgpt.tokens.use.direct is the plan permission itself. */
export const SCOPE = 'openid profile email offline_access resource.invoke chatgpt.tokens.use.direct';
/** The scope without which a token may sign in but not spend the plan. */
export const PLAN_SCOPE = 'chatgpt.tokens.use.direct';
/** The client a first sign-in registers through; the issued client id is kept and reused after it. */
export const DYNAMIC_CLIENT = 'dynamic_agent_client';
/** The name ChatGPT shows for this app when it registers. */
export const AGENT_NAME = 'Oya Browser';
/** The loopback path sign-in returns to: the one OpenAI's dynamic registration accepts. */
export const CALLBACK_PATH = '/auth/callback';
/** Loopback only: the browser that signs in is on this machine. */
export const LOOPBACK_HOST = '127.0.0.1';
/**
 * The loopback port sign-in returns to. OpenAI allows any port as long as one
 * attempt sends the same address twice; a fixed one (not Codex's 1455) is simply
 * predictable.
 */
export const LOOPBACK_PORT = 1456;
/** How long a sign-in may take before the listener gives up. */
export const LOGIN_TTL_MS = 600_000;
/** Refresh an access token this long before it expires. */
export const REFRESH_EARLY_MS = 300_000;
/** A token request gives up after this long. */
export const TOKEN_TIMEOUT_MS = 30_000;
/** A relayed model call gives up a little before the server stops waiting for it. */
export const RESPONSES_TIMEOUT_MS = 170_000;
/** Random bytes in a state, nonce or PKCE verifier. */
export const SECRET_BYTES = 32;
/** The sealed file the credential lives in, in the app's data folder. */
export const CREDENTIAL_FILE = 'chatgpt-credential.enc';
/** What the server stores as the model key when the credential is here; must match the server's DESKTOP_HELD. */
export const DESKTOP_HELD = 'chatgpt-desktop';
/** Milliseconds in a second, for expires_in. */
export const MS_PER_SECOND = 1000;
/** The longest proof-of-work search before giving up (difficulty 5 needs about a million). */
export const POW_MAX_TRIES = 50_000_000;
/** The page the browser tab shows once sign-in returns. */
export const CALLBACK_PAGE =
  '<!doctype html><meta charset="utf-8"><title>Oya Browser</title>' +
  '<body style="font:16px system-ui;max-width:32rem;margin:15vh auto;padding:0 1rem">' +
  '<h1>Back to Oya Browser</h1><p>You can close this tab.</p>';
/** What the app says when ChatGPT shared no email to make the account with. */
export const NO_EMAIL = 'ChatGPT did not share your email. Use "Have an Oya account?" to sign in instead.';
/** The plan Sign in with ChatGPT needs, said once so a change of tier is one edit. */
export const PLAN_NEEDED = 'Oya needs a ChatGPT Plus or Pro plan.';
/** What the app says when ChatGPT refuses to exchange a sign-in's code: most often an account without a plan that can be shared. */
export const SIGN_IN_REFUSED = `ChatGPT did not let this account connect. ${PLAN_NEEDED} Sign in again with an account that has one.`;
/** What the app says when the code exchange failed for a reason that is not the account's: OpenAI busy, rate-limited or unreachable. */
export const SIGN_IN_FAILED = 'ChatGPT could not finish the sign-in right now. Check your connection and try again.';
/** What the app says when ChatGPT's page comes back with an error: cancelled, or not allowed for this account. */
export const SIGN_IN_CANCELLED = `ChatGPT sign-in was cancelled or not allowed. ${PLAN_NEEDED} Sign in again with an account that has one.`;
/** What the app says when a token comes back without the permission to spend the plan. */
export const NO_PLAN_SCOPE = `ChatGPT did not let Oya use your plan. ${PLAN_NEEDED} Sign in again and allow "Use your ChatGPT plan".`;
/** What the app says when this network has used today's sign-ups. */
export const TOO_MANY_SIGNUPS =
  'Too many sign-ups from this network today. Use "Have an Oya account?" to sign in instead.';
/** HTTP OK, for the callback page. */
export const HTTP_OK = 200;
/** HTTP not found, for anything but the callback path. */
export const HTTP_NOT_FOUND = 404;
/** The first HTTP server-error status: a refusal at or above it is OpenAI's trouble, not the account's. */
export const HTTP_SERVER_ERROR = 500;
/** HTTP request timeout: the endpoint gave up waiting, not a refusal of the account. */
export const HTTP_TIMEOUT = 408;
/** HTTP too many requests: the server's sign-up limit for this network. */
export const HTTP_TOO_MANY = 429;
/** A model stream that sends nothing for this long has stalled: give up so the server retries it. */
export const STREAM_IDLE_MS = 45_000;
/** The events that end a Responses stream; reading stops at the first, whether or not the connection closes. */
export const STREAM_END = /"type":\s*"(response\.(completed|failed|incomplete)|error)"/;
/** Browsers the sign-in may open in when Oya is the default, most likely signed in to ChatGPT first; Safari is on every Mac. */
export const OTHER_MAC_BROWSERS = ['Google Chrome', 'Arc', 'Brave Browser', 'Microsoft Edge', 'Firefox', 'Safari'];
/** Opens a page in Edge, which every Windows has, whatever the default browser is. */
export const EDGE_SCHEME = 'microsoft-edge:';
/** What the app says when another program holds the sign-in's loopback port. */
export const PORT_BUSY = 'Another app is using the port ChatGPT sign-in returns to (1456). Close it and try again.';
/** Where a person gets a free Gemini API key, for the first-run key route. */
export const GEMINI_KEYS_URL = 'https://aistudio.google.com/apikey';
/** An email address, loosely: something@something.something, no spaces. */
export const EMAIL_SHAPE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
/** What the app says when the key route has no usable email. */
export const BAD_EMAIL = 'Enter your email address.';
/** What the app says when the key route has no key. */
export const NO_GEMINI_KEY = 'Paste your Gemini API key.';
