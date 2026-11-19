/** The recorded demo remains optional, accessible and recoverable without taking keyboard focus. */
import { readFileSync } from 'node:fs';
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RecordedDemo } from '@/app/_home/recorded-demo';

afterEach(cleanup);

it('autoplays the real recording muted with native controls and a walkthrough instead of a download link', () => {
  const { container } = render(<RecordedDemo />);
  const video = container.querySelector('video')!;
  expect(video.getAttribute('src')).toBe('/prior-auth.mp4');
  expect(video.getAttribute('poster')).toBe('/prior-auth-poster.jpg');
  expect(video.getAttribute('preload')).toBe('metadata');
  expect(video.controls).toBe(true);
  expect(video.hasAttribute('playsinline')).toBe(true);
  expect(video.autoplay).toBe(true);
  expect(video.muted).toBe(true);
  expect(video.loop).toBe(true);
  expect([video.width, video.height]).toEqual([1280, 860]);
  expect(screen.queryByText('Recorded demonstration · Synthetic data · No audio')).toBeNull();
  expect(screen.queryByText('See a prior authorization in action.')).toBeNull();
  expect(screen.queryByText('Watch a 35-second recording of a workflow in a mock payer portal.')).toBeNull();
  expect(screen.getByRole('region', { name: 'Product demo video' })).toBeTruthy();
  expect(screen.queryByRole('link', { name: 'Download video' })).toBeNull();
  expect(screen.getByRole('link', { name: 'Explore walkthrough' }).getAttribute('href')).toBe('#workflows');
  expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull();
  const media = readFileSync('public/prior-auth.mp4');
  expect(media.subarray(4, 8).toString()).toBe('ftyp');
});

it('announces errors and retries the same media while retaining focus through recovery and repeated failure', async () => {
  const user = userEvent.setup();
  const { container } = render(<RecordedDemo />);
  const video = container.querySelector('video')!;
  const load = vi.spyOn(video, 'load').mockImplementation(() => {});
  fireEvent.error(video);
  expect(screen.getByRole('status').textContent).toContain('The video couldn’t load.');
  const retry = screen.getByRole('button', { name: 'Try again' });
  retry.focus();
  await user.keyboard('{Enter}');
  expect(load).toHaveBeenCalledOnce();
  expect(document.activeElement).toBe(retry);
  expect(screen.getByRole('status').textContent).toBe('Reloading video…');
  fireEvent.loadedMetadata(video);
  expect(screen.getByRole('status').textContent).toBe('Video reloaded. Use the controls to play it.');
  expect(document.activeElement).toBe(retry);
  fireEvent.error(video);
  expect(screen.getByRole('status').textContent).toContain('The video couldn’t load.');
  expect(screen.getByRole('link', { name: 'Explore walkthrough' }).getAttribute('href')).toBe('#workflows');
});
