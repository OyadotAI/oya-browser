/** Only AWS-resolved buyer identity may enter a registration; it never proves activation. */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_IDENTIFIER } from './constants.ts';

/** Trusted identity obtained by exchanging the opaque registration token. */
export interface BuyerIdentity {
  /** The AWS buyer, shared across its licenses. */
  accountId: string;
  /** The purchased license, distinct from the buyer's account. */
  licenseArn: string;
  /** The configured seller product confirmed by AWS. */
  productCode: string;
}

/** A decrypted setup context; authority comes from authenticated encryption. */
export interface SetupContext extends BuyerIdentity {
  /** Prevents another encrypted record type being accepted here. */
  audience: string;
  /** Format ratchet for future incompatible changes. */
  version: number;
  /** Fresh context identity, without retaining the AWS token. */
  nonce: string;
  /** Absolute deadline, never extended by status reads. */
  expiresAt: number;
}

/** Public states intentionally have no activated/confirmed value yet. */
export interface SetupState {
  /** Opaque setup identity binds explicit consent to the context shown, across tabs. */
  contextId: string;
  /** Pending means linked, awaiting an authoritative activation integration. */
  status: 'ready' | 'pending';
  /** Only the last four digits, never an account login identity. */
  awsAccount: string;
}

/** A nonempty bounded opaque identifier. */
export const identifier = (value: unknown): value is string =>
  typeof value === 'string' && value.length > 0 && value.length <= MAX_IDENTIFIER && !/\s/.test(value);

/** Rejects incomplete and wrong-product responses, including legacy-only identities. */
export function validIdentity(value: BuyerIdentity, productCode: string) {
  return (
    typeof value?.accountId === 'string' &&
    /^\d{12}$/.test(value?.accountId) &&
    identifier(value?.licenseArn) &&
    value.licenseArn.startsWith('arn:') &&
    value.productCode === productCode
  );
}

/** Stable safe errors keep tokens and provider diagnostics out of responses. */
export const invalidSetup = () =>
  new HttpError(Status.BAD_REQUEST, 'Return to AWS Marketplace', { code: 'invalid_setup' });
/** Never reveal the account already owning a buyer or license. */
export const ownershipConflict = () =>
  new HttpError(Status.CONFLICT, 'This purchase is already connected', { code: 'purchase_conflict' });
