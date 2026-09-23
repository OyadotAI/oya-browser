/**
 * The sign-in page. A server component only so the captcha's site key is read
 * from this server's settings at request time; the page itself is LoginView.
 */
import { turnstileSiteKey } from '@/lib/captcha';
import { LoginView } from './login-view';

/** Sign in, then on to the console. */
export default function LoginPage() {
  return <LoginView siteKey={turnstileSiteKey()} />;
}
