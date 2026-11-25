/** Fixed buyer-facing copy separates a saved account connection from an AWS-confirmed subscription. */
import type { SetupKind } from './requests';

/** One heading and explanation per implemented state. Activation is deliberately not implemented. */
export const CONTENT: Record<SetupKind, [string, string]> = {
  loading: ['Checking your AWS purchase', 'This should only take a moment.'],
  'signed-out': ['Finish setting up Oya', 'Create an Oya account or sign in to connect your AWS Marketplace purchase.'],
  ready: ['Connect your AWS purchase', 'Connect this purchase to the Oya account shown below.'],
  pending: [
    'Waiting for AWS confirmation',
    'Your purchase is connected to your Oya account. AWS has not confirmed the subscription yet.',
  ],
  missing: [
    'Return to AWS Marketplace',
    'Open your Oya purchase in AWS Marketplace and choose the option to set up your account.',
  ],
  expired: [
    'Continue from AWS Marketplace',
    'This setup session has expired. Open your Oya purchase in AWS Marketplace to continue.',
  ],
  conflict: [
    'This purchase is already connected',
    'Sign in to the Oya account used to connect this purchase, or contact support.',
  ],
  changed: ['Review your AWS purchase', 'Another setup was opened. Check this purchase again before connecting it.'],
  unavailable: ['Setup is temporarily unavailable', 'Please try again later or contact support.'],
  offline: ['Connection interrupted', 'Check your connection, then try again.'],
};
