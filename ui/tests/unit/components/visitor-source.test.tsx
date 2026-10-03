/**
 * Unit tests for the visitor-source component: it notes the source of a visit
 * that lands on a public page, and not one that lands on the console.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';

let pathname = '/';
vi.mock('next/navigation', () => ({ usePathname: () => pathname }));
const rememberSource = vi.fn();
vi.mock('@/lib/visitor-source', () => ({ rememberSource: () => rememberSource() }));

const { VisitorSource } = await import('@/components/visitor-source');

describe('VisitorSource', () => {
  beforeEach(() => rememberSource.mockClear());

  it('notes where a visit to a public page came from', () => {
    pathname = '/';
    render(<VisitorSource />);
    expect(rememberSource).toHaveBeenCalledOnce();
  });

  it('notes nothing for a visit that lands on the console', () => {
    pathname = '/dashboard';
    render(<VisitorSource />);
    expect(rememberSource).not.toHaveBeenCalled();
  });
});
