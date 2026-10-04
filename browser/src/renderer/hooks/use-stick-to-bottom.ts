/**
 * Keeps a scrolling list at its newest row while the person is already at the
 * bottom: a scroll away lets go, a scroll back to the bottom follows again.
 */
import { useLayoutEffect, useRef } from 'react';
import { RendererConstants as C } from '../core/constants.ts';

/** Whether `el` is scrolled to within the stick distance of its bottom. */
const nearBottom = (el: HTMLElement): boolean => el.scrollHeight - el.scrollTop - el.clientHeight < C.NET_LOG_STICK_PX;

/** A ref for the list and its scroll handler; scrolls to the bottom after `changed` changes while stuck there. */
export function useStickToBottom<T extends HTMLElement>(changed: unknown) {
  const box = useRef<T>(null);
  const stuck = useRef(true);
  const onScroll = () => void (box.current && (stuck.current = nearBottom(box.current)));
  useLayoutEffect(() => {
    if (box.current && stuck.current) box.current.scrollTop = box.current.scrollHeight;
  }, [changed]);
  return { box, onScroll };
}
