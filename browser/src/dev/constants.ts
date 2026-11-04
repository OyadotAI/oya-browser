/** Development launch defaults keep private logs and process outcomes explicit. */
/** First user argument after the Node launcher path. */
export const USER_ARGS_START = 2;
/** Only the current user can read application startup output. */
export const DEVELOPMENT_LOG_MODE = 0o600;
/** Launch failures and interrupted Launch Services waiters report failure. */
export const LAUNCH_FAILURE = 1;
