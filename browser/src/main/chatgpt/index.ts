/**
 * Sign in with ChatGPT on this machine: the sign-in, the relay that makes the
 * model calls for the server's agent, and the first-run onboarding built on it.
 */
export { ChatgptAuth } from './auth.ts';
export { Onboarding, type OnboardingStatus } from './onboarding.ts';
export { answerModelRequest } from './relay.ts';
export { SetupAttempt } from './attempt.ts';
export { openOutside } from './outside.ts';
