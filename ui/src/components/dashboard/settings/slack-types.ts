/**
 * Slack settings as the server returns them.
 */

/** GET/PUT/DELETE /slack. */
export type SlackConfig = {
  /** A workspace is connected. */
  connected: boolean;
  /** This deployment has a Slack app of its own to install. */
  oauthAvailable: boolean;
  /** The OAuth redirect this console uses, for the Slack app's allow-list. */
  redirectUri: string | null;
  /** The connected workspace. */
  teamName: string | null;
  /** The alert channel. */
  channelId: string | null;
  /** The alert channel's name. */
  channelName: string | null;
  /** Connected with the customer's own bot token. */
  byo: boolean;
  /** The bot token, masked. */
  botToken: string;
};

/** A channel the bot can post to. */
export type Channel = {
  /** Slack channel id. */
  id: string;
  /** Name without the #. */
  name: string;
  /** Private channels need the bot invited. */
  private: boolean;
};

/** A PUT /slack body: a new bot token, or a channel (null clears it). */
export type SlackSave = {
  /** A bot token to verify and store. */
  botToken?: string;
  /** The alert channel. */
  channelId?: string | null;
};

/** GET /slack/channels. */
export type ChannelList = {
  /** Channels the bot can see. */
  channels: Channel[];
};

/** GET /slack/install?json=1. */
export type InstallLink = {
  /** Slack's OAuth install page. */
  url: string;
};
