/** Registration is opt-in and requires a shared encryption key and explicit seller product. */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { AWS_REGION } from './constants.ts';

/** The nonsecret settings every registration stage uses. */
export interface MarketplaceConfig {
  /** The one seller product this deployment accepts. */
  productCode: string;
  /** Where ResolveCustomer runs with the seller's IAM identity. */
  region: string;
  /** Fixed public origin used for cookies and CSRF checks. */
  origin: string;
}

/** A safe refusal, never the contents of operator settings. */
export const unavailable = () =>
  new HttpError(Status.UNAVAILABLE, 'Setup is temporarily unavailable', { code: 'unavailable' });

/** Reads registration settings in one place; a generated disk key cannot support replicas. */
export function marketplaceConfig(consoleUrl: string): MarketplaceConfig {
  const productCode = process.env.OYA_AWS_MARKETPLACE_PRODUCT_CODE?.trim();
  if (process.env.OYA_AWS_MARKETPLACE_ENABLED !== 'true' || !productCode || !process.env.OYA_PROFILE_SECRET)
    throw unavailable();
  const origin = safeOrigin(consoleUrl);
  const region = process.env.OYA_AWS_MARKETPLACE_REGION?.trim() || AWS_REGION;
  return { productCode, region, origin };
}

/** HTTPS is required except for an explicitly enabled loopback development server. */
function safeOrigin(value: string) {
  const url = parseOrigin(value);
  const local = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
  if (url.username || url.password || (url.protocol !== 'https:' && !(local && url.protocol === 'http:')))
    throw unavailable();
  return url.origin;
}

/** Invalid URLs must not expose operator input through an exception. */
function parseOrigin(value: string) {
  try {
    return new URL(value);
  } catch {
    throw unavailable();
  }
}
