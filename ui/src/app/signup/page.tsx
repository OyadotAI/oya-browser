/**
 * The sign-up page. A server component only so the captcha's site key is read
 * from this server's settings at request time; the page itself is SignupView.
 */
import { turnstileSiteKey } from '@/lib/captcha';
import { SignupView } from './signup-view';

/** Create an account. */
export default function SignupPage() {
  return <SignupView siteKey={turnstileSiteKey()} />;
}
