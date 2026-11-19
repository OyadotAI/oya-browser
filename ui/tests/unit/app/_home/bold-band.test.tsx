/** The coral band draws its words several times so the loop has no seam, and reads them out once. */
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { LoudBand } from '@/app/_home/bold-band';
import { BAND } from '@/app/_home/bold-content';
import { BAND_COPIES } from '@/app/_home/constants';

afterEach(cleanup);

describe('LoudBand', () => {
  it('reads each phrase out once', () => {
    render(<LoudBand />);
    expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual(BAND);
  });

  it('draws hidden copies for the seamless loop', () => {
    const { container } = render(<LoudBand />);
    const lists = container.querySelectorAll('ul');
    expect(lists).toHaveLength(BAND_COPIES);
    expect(lists[1].getAttribute('aria-hidden')).toBe('true');
    expect(lists[1].textContent).toBe(BAND.join(''));
  });
});
