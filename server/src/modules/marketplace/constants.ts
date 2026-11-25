/** Bounds and protocol names keep AWS registration separate from subscription activation. */

/** The buyer-facing handoff and return destination. */
export const FULFILLMENT_PATH = '/marketplace/aws';
/** The form field AWS posts, never an HTTP header. */
export const TOKEN_FIELD = 'x-amzn-marketplace-token';
/** The optional AWS free-trial hint carries no authority. */
export const OFFER_FIELD = 'x-amzn-marketplace-offer-type';
/** Maximum callback body, independently of task-file uploads. */
export const FORM_LIMIT_BYTES = 65_536;
/** Bounds repeated/extra form fields before parsing. */
export const FORM_PARAMETER_LIMIT = 4;
/** Oya's continuation lifetime, independent of AWS token expiry. */
export const CONTINUATION_MS = 1_800_000;
/** A single server-side resolution deadline. */
export const AWS_TIMEOUT_MS = 8000;
/** Bounded SDK retries, including the initial attempt. */
export const AWS_ATTEMPTS = 2;
/** Avoid accumulating credential/network work during an AWS outage. */
export const AWS_CONCURRENCY = 4;
/** Marketplace Metering's default supported region. */
export const AWS_REGION = 'us-east-1';
/** Bounds opaque provider identifiers without interpreting their contents. */
export const MAX_IDENTIFIER = 1024;
/** AWS registration tokens have a separate request bound. */
export const MAX_TOKEN = 16_384;
/** Bound cookies before attempting decryption. */
export const MAX_COOKIE = 4096;
/** Only a suffix is returned to the buyer's page. */
export const ACCOUNT_SUFFIX = 4;
/** The encrypted host-only continuation cookie. */
export const CONTINUATION_COOKIE = 'oya_aws_setup';
/** Readable navigation hint, carrying no buyer identity. */
export const PENDING_COOKIE = 'oya_aws_pending';
/** Domain separation prevents reuse of another sealed record as a purchase. */
export const CONTINUATION_SCOPE = 'aws-marketplace-fulfillment:v1';
/** The supported continuation format. */
export const CONTINUATION_VERSION = 1;
