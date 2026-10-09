/** Fixed native recording capability; never expose a general IPC sender to website worlds. */
export const NATIVE_RECORDING = {
  /** Matches the browser-owned analyzer world, not the main or preload world. */
  WORLD_ID: 1004,
  /** Four random words give each preload document a fresh 128-bit identity. */
  DOCUMENT_TOKEN_WORDS: 4,
  /** Only this channel can be reached through the isolated bridge. */
  CHANNEL: 'oya-native-recording-batch',
  /** The bridge is visible only inside the agent world. */
  BINDING: '__oyaNativeRecording',
  /** Bound individual transfers before they enter the browser process. */
  MAX_PAYLOAD_CHARS: 262144,
  /** Recording epochs are short browser-generated identifiers, not arbitrary data. */
  MAX_EPOCH_CHARS: 128,
  /** Bound native CSS selector parsing from the isolated recording world. */
  MAX_SELECTOR_CHARS: 4096,
  /** Bound retained document authorizations and live frame subscriptions per recording. */
  MAX_DOCUMENTS: 1024,
} as const;
