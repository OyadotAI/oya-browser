# Stealth, measured

The README links here rather than carrying this, because a number nobody can reproduce is
marketing and a number they can is a document.

## The measurement

> **"Zero detection" is neither measurable nor achievable**: the published leader sits near 77% bypass. This produces a number instead.
>,
> [`server/tests/integration/stealth.test.js`](../server/tests/integration/stealth.test.js)

The same headless Chrome, launched twice: once bare, once with a persona applied exactly as production does. Both then face the public detectors. Chrome 153.0.8010.52 on macOS, re-run 2026-09-20:

| | Bare headless Chrome | With an Oya persona |
|:---|:---:|:---:|
| **CreepJS headless score** (lower is better) | 100% | **0%** |
| **CreepJS stealth-tampering score** (lower is better) | 0% | **0%** |
| **Bot.Sannysoft** | 27 / 31 pass | **31 / 31 pass** |
| **Oya probe suite** (29 probes, weighted, 64 points) | 55 / 64 | **64 / 64** |
| CreepJS like-headless score (lower is better) | 31% | 25% |

A CreepJS lies count was read as 0 on 2026-09-11 and was not on the page the harness could read
on 2026-09-20, for either browser, so it is left out of the table rather than carried forward.
What the run does still show is that nothing the detectors score caught the persona. Why it holds:

- 🧩 **Native first.** Chrome emulates the webdriver flag, platform, core count, locale, timezone and screen itself over CDP. There is no patched value to catch.
- 🎭 **Native-shaped patches.** What emulation can't reach is patched to look native from every realm: not constructible, no `prototype`, `[native code]` in every frame, "Illegal invocation" off the prototype, including CreepJS's phantom iframe.
- 🧵 **Workers and iframes too.** Dedicated, shared and service workers plus cross-site iframes, where CAPTCHA widgets live. Each is held paused until covered, so page and workers report one machine.

Of the remaining like-headless signals, two are headless-only rendering defaults; the other three are Android-only APIs real desktop Chrome lacks as well. Don't take our word for it:

```bash
oya stealth-test --live                  # from a checkout
cd server && npm run stealth -- --live   # or run the harness directly
```

One caveat worth stating plainly, because it cuts against selling this: **a residential proxy has
no place on a regulated path.** It puts a third party in the middle of a request that may carry
patient data, and no residential-proxy vendor will sign a BAA.
[`compliance/EVIDENCE.md`](../compliance/EVIDENCE.md) records §164.308(b)(1) as passing *because*
egress is direct or customer-owned, and the check fails if a proxy is configured. Stealth is a
scraping virtue; on a portal your customer has a contract with, the thing that matters is the
audit trail.
