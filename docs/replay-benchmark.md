# How well a recorded run replays

Ten public sites, recorded once by the agent and then replayed with **no model in the loop and
auto-healing switched off**, so a step that no longer fits fails loudly instead of being repaired
behind the scenes. Run on 2026-09-20 against a self-hosted stack on one Mac, Chrome 153.

Healing off is the point: with it on, the agent papers over a gap and the run still succeeds,
which tells you nothing about the recording.

| Site | What the flow did | Steps | Record | Replay | Result |
|:--|:--|--:|--:|--:|:--|
| wikipedia | follow two article links | 3 | 11.8s | **10.6s** | every step |
| hackernews | open a story's comments, return via the logo | 3 | 11.7s | **8.5s** | every step |
| github | switch between Issues and Pull requests | 4 | 29.0s | **17.5s** | every step |
| mdn | follow two interface links | 7 | 43.1s | **23.3s** | every step |
| stackoverflow | open a question, then its first tag | 16 | 264.6s | **111.8s** | every step |
| linkedin | page through search results | 8 | 20.7s | **25.0s** | every step |
| amazon | paginate a search twice | 8 | 118.3s | **116.0s** | every step |
| airbnb | note a listing, then the next page | 5 | 20.9s | **15.6s** | every step |
| reddit | read a post, open its subreddit | 2 | 58.5s | **36.8s** | every step |
| **ebay** | paginate a search twice | 5 | 139.0s | 61.2s | **stopped at step 3** |

**Nine of ten replayed every recorded step.** No fallback to the agent, no healing, and every
replay inside two minutes.

## What stopped the tenth

eBay's own pagination link redirected the click to `signup.ebay.com/pa/crte`, a registration wall.
The replay noticed that the element it reached was not the element it recorded and stopped:

```
Step 3 of 5 (click) failed: a[href="…&_pgn=2"] no longer points at "2",
  the page is now "Register: Create a personal eBay account"
```

That is the intended behaviour. The alternative, carrying on and typing into whatever page it
landed on, is how automation fills in a stranger's form. eBay's earlier refusal was a plain `403`
on the search URL before any automation-shaped behaviour, and it is intermittent: the same flow
recorded cleanly on this run and failed on an earlier one.

## Reading the numbers honestly

- **Replay is paced on purpose**, 350–1200ms between steps and longer after a page changes. It is
  not "instant", and it declines to be: a faster replay on Amazon drew a challenge that a slower
  one did not.
- **Amazon is the slow one** (14.5s a step against ~3.5s elsewhere) because its pagination links
  carry a per-visit request id, so that link cannot be matched literally and each click needs a
  fresh look at the page first. Correctness cost speed there, deliberately.
- **A step count is not a difficulty score.** Two steps on reddit took longer to record than eight
  on LinkedIn.
- **This is ten sites on one machine on one day.** It is a measurement, not a guarantee, and the
  numbers move with the sites.

## Reproducing it

The harness lives in the repository's QA folder, which is not published, because its other cases
carry customer portal names. The shape is: record with an LLM run, save it as a playbook, replay
that playbook with `autoHeal: false`, and compare the steps completed against the steps recorded.
Everything it uses is in the public API, `browser.ask()`, `browser.toPlaybook()`,
`browser.play()`, so the same table can be produced against any set of sites you choose.
