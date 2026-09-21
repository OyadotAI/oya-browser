/**
 * Unit tests for the Slack panel: it loads the workspace and its channels,
 * saves a token or channel straight away, disconnects, and shows failures.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (importActual) => ({
  ...(await importActual<typeof import('@/lib/api-client')>()),
  api: vi.fn(),
}));

import { api } from '@/lib/api-client';
import SlackSection, { type SlackConfig } from '@/components/dashboard/slack-section';
import { Row } from '@/components/dashboard/settings/fields';
import { ToastProvider } from '@/components/dashboard/toast';

const apiMock = vi.mocked(api);

/** A Slack config, not connected unless overridden. */
const slack = (over: Partial<SlackConfig> = {}): SlackConfig => ({
  connected: false,
  oauthAvailable: false,
  redirectUri: null,
  teamName: null,
  channelId: null,
  channelName: null,
  byo: false,
  botToken: '',
  ...over,
});

const CHANNELS = [
  { id: 'C1', name: 'alerts', private: false },
  { id: 'C2', name: 'ops', private: true },
];

/** Answers each Slack endpoint; `fail` makes one path reject. */
function serve(config: SlackConfig, fail?: string) {
  apiMock.mockImplementation(async (path, opts) => {
    if (path === fail) throw new Error(`${path} failed`);
    if (path === '/slack/channels') return { channels: CHANNELS };
    if (opts?.method === 'DELETE') return slack();
    return opts?.method === 'PUT' ? { ...config, ...(opts.body as object), connected: true } : config;
  });
}

/** Renders the panel. */
function setup() {
  render(
    <ToastProvider>
      <SlackSection apiKey="k" Row={Row} />
    </ToastProvider>,
  );
}

describe('SlackSection', () => {
  afterEach(cleanup);

  it('offers the token form as the only path when there is no Slack app', async () => {
    serve(slack());
    setup();
    await userEvent.type(await screen.findByLabelText('Bot token'), '  xoxb-1  ');
    await userEvent.click(screen.getByRole('button', { name: 'Connect' }));
    expect(apiMock).toHaveBeenCalledWith('/slack', { key: 'k', method: 'PUT', body: { botToken: 'xoxb-1' } });
    expect(await screen.findByText('Slack connected')).toBeTruthy();
  });

  it('offers the one-click install when the deployment has a Slack app', async () => {
    serve(slack({ oauthAvailable: true, redirectUri: 'https://c/cb' }));
    setup();
    expect(await screen.findByRole('button', { name: 'Connect Slack' })).toBeTruthy();
    expect(screen.getByText('Use your own Slack app instead')).toBeTruthy();
    expect(screen.getByText('https://c/cb')).toBeTruthy();
  });

  it('loads channels once connected and saves a chosen channel immediately', async () => {
    serve(slack({ connected: true, teamName: 'Acme', botToken: 'xoxb-…1' }));
    setup();
    const select = await screen.findByLabelText('Channel for Oya alerts');
    await waitFor(() => expect(screen.getByRole('option', { name: '# alerts' })).toBeTruthy());
    expect(screen.getByText('Nothing is sent until you pick one.')).toBeTruthy();
    await userEvent.selectOptions(select, 'C1');
    expect(apiMock).toHaveBeenCalledWith('/slack', { key: 'k', method: 'PUT', body: { channelId: 'C1' } });
    expect(await screen.findByText('Alerts go to #alerts.')).toBeTruthy();
  });

  it('tells the user to invite the bot to a private channel', async () => {
    serve(slack({ connected: true, channelId: 'C2' }));
    setup();
    expect(await screen.findByText('/invite @Oya')).toBeTruthy();
  });

  it('shows a channel-load failure and leaves the list empty', async () => {
    serve(slack({ connected: true }), '/slack/channels');
    setup();
    expect((await screen.findByRole('alert')).textContent).toBe('/slack/channels failed');
    expect(screen.getByRole('option', { name: 'Choose a channel…' })).toBeTruthy();
  });

  it('disconnects the workspace', async () => {
    serve(slack({ connected: true, teamName: 'Acme' }));
    setup();
    await userEvent.click(await screen.findByRole('button', { name: 'Disconnect Slack' }));
    expect(apiMock).toHaveBeenCalledWith('/slack', { key: 'k', method: 'DELETE' });
    expect(await screen.findByText('Available once your workspace is connected.')).toBeTruthy();
  });

  it('shows why saving a token failed', async () => {
    serve(slack(), '/slack');
    apiMock.mockResolvedValueOnce(slack());
    setup();
    await userEvent.type(await screen.findByLabelText('Bot token'), 'bad');
    await userEvent.click(screen.getByRole('button', { name: 'Connect' }));
    expect((await screen.findByRole('alert')).textContent).toBe('/slack failed');
  });
});
