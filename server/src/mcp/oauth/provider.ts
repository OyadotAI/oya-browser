/**
 * The OAuth server that lets claude.ai, and any MCP client that signs in with
 * OAuth, reach the pool endpoint without an API key. The SDK serves the
 * endpoints and checks PKCE and redirect URIs; this keeps their state, sends the
 * person to the console to allow the connection, and hands out a project
 * credential as the access token, so every check past sign-in is the usual one.
 */
import type { Response } from 'express';
import type { AuthorizationParams, OAuthServerProvider } from '@modelcontextprotocol/sdk/server/auth/provider.js';
import type { OAuthClientInformationFull, OAuthTokenRevocationRequest } from '@modelcontextprotocol/sdk/shared/auth.js';
import {
  InvalidClientMetadataError,
  InvalidGrantError,
  InvalidTokenError,
} from '@modelcontextprotocol/sdk/server/auth/errors.js';
import { authenticateToken } from '../../modules/auth/service.ts';
import { control } from '../../modules/control/service.ts';
import { roleIn } from '../../modules/control/membership.ts';
import { consoleUrl } from '../../modules/slack/service.ts';
import { Status } from '../../platform/http-status.ts';
import {
  CONNECT_PAGE,
  CONNECTOR_MAX_ROLE,
  CONNECTOR_SUFFIX,
  DEFAULT_CLIENT_NAME,
  INVALID_GRANT,
  MAX_CLIENT_NAME,
  MAX_REDIRECT_URIS,
} from './constants.ts';
import { getClient, openRequest, peekCode, putClient, spendCode, touchClient, type Grant } from './store.ts';

/** The grant behind a code, if it was issued to this client (for this redirect, when one is named); throws invalid_grant otherwise. */
function ownGrant(grant: Grant | null, client: OAuthClientInformationFull, redirectUri?: string) {
  if (!grant || grant.clientId !== client.client_id) throw new InvalidGrantError(INVALID_GRANT);
  if (redirectUri && redirectUri !== grant.redirectUri) throw new InvalidGrantError(INVALID_GRANT);
  return grant;
}

/** The principal a token stands for, or null when it is unknown, revoked or expired. */
const principalOf = (token: string) => authenticateToken(token).catch(() => null);

/** The credential's name in the project's list, so the person can tell which app holds it. */
const labelFor = (client: OAuthClientInformationFull) =>
  `${client.client_name || DEFAULT_CLIENT_NAME}${CONNECTOR_SUFFIX}`;

/** Loopback hosts a native app may answer on over plain HTTP. */
const LOOPBACK = ['localhost', '127.0.0.1', '[::1]'];

/** Whether the console may send a person's browser to `uri`: HTTPS, or HTTP on loopback; never javascript: or data:. */
function safeRedirect(uri: string) {
  const url = URL.parse(uri);
  return url?.protocol === 'https:' || (url?.protocol === 'http:' && LOOPBACK.includes(url.hostname));
}

/** Registers a client whose every redirect URI is safe to send a browser to; the console follows them. */
async function registerClient(client: Parameters<typeof putClient>[0]) {
  if (!client.redirect_uris.every(safeRedirect))
    throw new InvalidClientMetadataError('redirect_uris must be https, or http on localhost');
  if (client.redirect_uris.length > MAX_REDIRECT_URIS)
    throw new InvalidClientMetadataError(`at most ${MAX_REDIRECT_URIS} redirect_uris`);
  if ((client.client_name || '').length > MAX_CLIENT_NAME)
    throw new InvalidClientMetadataError(`client_name is at most ${MAX_CLIENT_NAME} characters`);
  return putClient(client);
}

/**
 * The credential a grant buys, until revoked. An owner's is an operator's, not an
 * administrator's: an MCP client drives browsers and never needs to change
 * settings or credentials, so a leaked token cannot either; nothing binds it to
 * the owner, who cannot be removed from their own project. A member's carries
 * their role and is bound to their membership, so it ends when they leave. A
 * project deleted since the person allowed it is an invalid grant.
 */
async function credentialFor(grant: Grant, label: string) {
  const { project, role } = await roleIn(grant.userId, grant.projectId);
  if (!project || !role) throw new InvalidGrantError(INVALID_GRANT);
  const owner = project.ownerUser === grant.userId;
  const options = { role: owner ? CONNECTOR_MAX_ROLE : role, label, expiresAt: null, connector: true };
  return control().credential(control().projectKey(project), options, owner ? null : grant.userId);
}

/** Oya's side of the SDK's OAuth server. */
export const provider: OAuthServerProvider = {
  clientsStore: { getClient, registerClient },

  /** Keeps the request and sends the person to the console page that asks them to allow it. */
  async authorize(client: OAuthClientInformationFull, params: AuthorizationParams, res: Response) {
    const { redirectUri, codeChallenge, state } = params;
    const id = await openRequest({ clientId: client.client_id, redirectUri, codeChallenge, state });
    await touchClient(client.client_id);
    res.redirect(Status.FOUND, `${consoleUrl()}${CONNECT_PAGE}?request=${encodeURIComponent(id)}`);
  },

  /** The PKCE challenge a code is bound to, read without spending the code. */
  async challengeForAuthorizationCode(client: OAuthClientInformationFull, code: string) {
    return ownGrant(await peekCode(code), client).codeChallenge;
  },

  /** Spends the code and mints the person's credential for the project they chose. */
  async exchangeAuthorizationCode(
    client: OAuthClientInformationFull,
    code: string,
    _verifier?: string,
    redirectUri?: string,
  ) {
    const grant = ownGrant(await spendCode(code), client, redirectUri);
    const { token } = await credentialFor(grant, labelFor(client));
    return { access_token: token, token_type: 'bearer' };
  },

  /** No refresh tokens are issued: the credential lasts until it is revoked. */
  async exchangeRefreshToken(): Promise<never> {
    throw new InvalidGrantError('Refresh tokens are not issued; connect again');
  },

  /** A token is good while the credential it names is. */
  async verifyAccessToken(token: string) {
    if (!(await principalOf(token))) throw new InvalidTokenError('Invalid, expired or revoked token');
    return { token, clientId: '', scopes: [] };
  },

  /** Revokes the credential behind a token; an unknown one is left alone, as OAuth asks. */
  async revokeToken(_client: OAuthClientInformationFull, { token }: OAuthTokenRevocationRequest) {
    const principal = await principalOf(token);
    if (principal?.credentialId) await control().revoke(principal.key, principal.credentialId);
  },
};
