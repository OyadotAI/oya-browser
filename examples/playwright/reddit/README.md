# 🔎 Reddit at Scale with Oya

> 🤖 **10 real browsers, 10 Reddit searches, all in parallel.** Plain Playwright drives Oya browsers that look like people at home, not bots in a datacenter.

---

## ✨ What it does

For each of 10 searches about browser automation, the script:

1. 🔍 **Searches** reddit.com and keeps the top 5 results.
2. 📖 **Opens** the top 3 posts.
3. 💬 **Saves** each post's subreddit, score, comment count, date, body text and top 5 comments.
4. 💾 **Writes** everything to `results.json`.

🪶 Images, video and fonts are skipped, because residential proxies bill per GB.

---

## 🧪 Why a real browser *and* a residential IP

Reddit is hard to scrape. Here is what we tried:

| Setup | Result |
|:---|:---|
| 🖥️ Plain Playwright, home IP | ❌ "Prove your humanity" challenge |
| 🕶️ Headless Chrome, residential proxy | ❌ "Blocked by network security" |
| ☁️ Oya cloud browser, no proxy | ❌ "whoa there, pardner!" on every request |
| 🛡️ **Oya browser, residential proxy** | ✅ **Challenge passed, 10/10 searches** |

A good IP alone is not enough, and neither is a good browser. **It takes both.** 🤝

> ℹ️ Old Reddit now requires an account, so the script reads the new site.

---

## 🛡️ Before you run: give a profile a residential proxy

A profile (persona) is one device: fingerprint, cookies and proxy, bound together for life. Returning runs look like the same person coming back. 👤

### 🖱️ In the dashboard

1. Open **Profiles** and click **🌐 Proxies**.
2. Paste the proxy URL from your vendor and click **Add proxy**. Then click **Check all** to see its real exit IP.
3. Open your profile, pick the proxy under **Exit proxy**, and click **Apply**.

### 🧑‍💻 Or with the SDK

```ts
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
const proxy = await oya.proxies.create({ url: process.env.PROXY_URL!, geo: 'US', label: 'reddit-us' });
const [profile] = (await oya.personas.list()).filter((p) => p.isDefault);
await oya.personas.pinProxy(profile.id, proxy.id);
```

### 📌 Proxy tips

- 🏠 **Buy residential or mobile.** Any residential vendor works: IPRoyal, Decodo, Oxylabs, Bright Data and others. Datacenter IPs get blocked.
- 🔗 **Use http or https.** Chromium ignores SOCKS5 passwords, so Oya refuses them.
- 📍 **Make it sticky.** A rotating proxy gives every connection a new IP, which is slower and looks less like one person. Vendors add a session to the credentials. For IPRoyal, that goes at the end of the password:

  ```
  http://USER:PASS_country-us_session-reddit01_lifetime-24h@geo.iproyal.com:12321
  ```

---

## 🚀 Run it

From the `examples/` folder:

```bash
npm install
cp .env.example .env        # paste your OYA_API_KEY
unset OYA_API_KEY           # a key exported in your shell beats .env
npx tsx --env-file=.env playwright/reddit/scrape.ts
```

You'll see something like:

```
10/10 searches ok in 49s
  ✓ cloud browser automation: 3 posts
  ✓ browserbase alternative: 3 posts
  ...
```

### ⚙️ Settings

| Variable | Default | What it does |
|:---|:---|:---|
| `OYA_API_KEY` | required | Your key from [oyabrowser.com](https://oyabrowser.com) |
| `OYA_PERSONA` | `default` | The profile to run as. Pin the proxy to this one. |
| `OYA_BROWSER_ID` | unset | Attach to one running browser instead of starting 10. See below. |
| `OYA_BASE_URL` | hosted service | Your server, if self-hosted |

---

## 🖥️ Use a browser that's already running

Set `OYA_BROWSER_ID` to reuse one open Oya browser, such as the desktop app. The searches then run one after another, and the browser stays open afterwards.

The browser must allow CDP connections, so start the desktop app with a debugging port:

```bash
OYA_REMOTE_DEBUGGING_PORT=9222 open -a "Oya Browser"
```

Then find its id in the dashboard, or with `oya.browser.list()`.

---

## 📦 Output

`results.json` holds one entry per search:

```json
{
  "query": "anti-detect browser",
  "hits": [{ "title": "Best antidetect browser?", "url": "https://www.reddit.com/r/..." }],
  "posts": [{
    "title": "Best antidetect browser?",
    "url": "https://www.reddit.com/r/...",
    "subreddit": "r/antidetectbrowser",
    "score": 120,
    "comments": 34,
    "created": "2026-05-14T09:12:00.000000+0000",
    "body": "Been digging through all the lists...",
    "topComments": [{ "score": 2, "text": "the proxy you use matters way more than the browser." }]
  }]
}
```

A failed search has an `error` instead. If every search fails, the previous `results.json` is kept. 🛟

---

## 🎛️ Make it yours

At the top of `scrape.ts`:

- 📝 `QUERIES`: what to search for. Each query gets its own cloud browser.
- 🔢 `HITS`, `POSTS`, `COMMENTS`: how much to keep. Fewer posts means less proxy bandwidth.

---

## 🧯 Troubleshooting

| You see | What it means | Fix |
|:---|:---|:---|
| `No CDP URL` | The server or browser can't take Playwright yet | Update your Oya server, or set `OYA_REMOTE_DEBUGGING_PORT` on the browser |
| `ERR_TUNNEL_CONNECTION_FAILED` | The proxy refused the connection | Click **Check all** in Proxies. `proxy answered 402` means your proxy balance ran out 💸 |
| `no results (Reddit - Prove your humanity)` | Reddit flagged the traffic | Make sure the profile has a residential proxy pinned |
| A different IP on every site | The proxy is rotating | Add a sticky session to the proxy URL 📍 |
| Very slow runs | Rotating IPs reconnect constantly | Same fix: a sticky session |

---

## 📝 Notes

- 🧹 **Nothing is left running.** Each cloud browser stops when its search ends, even on failure.
- 🔓 **No Reddit login.** Ten browsers on one account is an obvious bot signal.
- 👥 **One profile, ten browsers.** They share a device and an exit IP. For stricter sites, give each browser its own profile and sticky session.
