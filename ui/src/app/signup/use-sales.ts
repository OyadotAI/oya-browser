/**
 * The enterprise path's state: the answers, which step is showing, and the
 * booked signal Calendly sends from its frame.
 */
'use client';

import { useEffect, useState, type FormEvent } from 'react';
import { event } from '@/lib/analytics';
import { usePatchState } from '@/lib/hooks/use-patch-state';
import { EMPTY_DETAILS, isBooked, salesProblem, type SalesDetails } from './sales';

/** Where the visitor is: answering, picking a time, or booked. */
export type SalesStep = 'details' | 'schedule' | 'booked';

/** Moves to 'booked' when Calendly reports the call, once the calendar is showing. */
function useBookedSignal(step: SalesStep, setStep: (s: SalesStep) => void) {
  useEffect(() => {
    if (step !== 'schedule') return;
    const onMessage = (e: MessageEvent) => isBooked(e) && (event('sales_call_booked'), setStep('booked'));
    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, [step, setStep]);
}

/** On to the calendar, when the answers are enough to book with. */
function advance(details: SalesDetails, setStep: (s: SalesStep) => void) {
  if (!salesProblem(details)) setStep('schedule');
}

/** The answers, the step, the error line and the handlers that move between steps. */
export function useSales() {
  const [details, patch] = usePatchState(EMPTY_DETAILS);
  const [step, setStep] = useState<SalesStep>('details');
  const [error, setError] = useState('');
  useBookedSignal(step, setStep);
  const submit = (e: FormEvent) => (e.preventDefault(), setError(salesProblem(details)), advance(details, setStep));
  return { details, patch, step, error, submit, edit: () => setStep('details') };
}

/** What useSales returns. */
export type Sales = ReturnType<typeof useSales>;
