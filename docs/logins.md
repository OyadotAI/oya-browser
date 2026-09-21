# Logins: getting them in, and taking them anywhere

A persona's cookie jar is its logins. Every browser that runs as the persona,
on your desktop or in the cloud, shares that jar, so you sign in once. This page
covers every way to get logins into a persona and out of one.

## Which way do I want?

| You have                                                       | You want                                  | Do this                                                          |
| :------------------------------------------------------------- | :---------------------------------------- | :--------------------------------------------------------------- |
| Logins in Chrome, Firefox, Arc, Brave or Edge on this computer | Them in Oya                               | [Import from a browser](#import-from-a-browser-on-this-computer) |
| Nothing yet                                                    | To sign in once                           | [Sign in on the desktop](#sign-in-on-the-desktop)                |
| One persona signed in                                          | Another persona signed in too             | [Copy between personas](#copy-between-personas)                  |
| A persona signed in                                            | The session in a script, Playwright, curl | [Export](#export)                                                |
| A cookie file from anywhere                                    | Those logins in a persona                 | [Import a file](#import-a-file)                                  |

## Import from a browser on this computer

In Oya Browser, click the connection pill at the top right (it says
**Connected**), then under **Import logins** pick the browser and press
**Import**. Your default browser is listed first.

What happens:

- Each profile of that browser becomes a persona, carrying that profile's
  cookies. The last profile you used becomes the persona this desktop runs as,
  and the desktop reconnects as it.
- Chrome, Arc, Brave and Edge are read by launching the browser itself,
  headless, on a **copy** of the profile, so it decrypts its own cookies. Your
  running browser is never touched. Saved passwords are not copied anywhere.
- Firefox is read straight from its cookie database, including the logins of
  the last few hours that a running Firefox has not written back yet. This needs
  the `sqlite3` command, which macOS ships.
- The persona presents this app's own browser engine version, whatever the
  source browser was. Claiming a different Chrome than the one running is
  something sites can check.

Some sessions will not survive the move, by the site's own design: a session
bound to the device it was made on (some banks, and Google on recent Chrome for
Windows) asks you to sign in again. Sign in once in Oya and it sticks from
there.

Supported: macOS and Windows. On Linux, use [Import a file](#import-a-file).

## Sign in on the desktop

Open the site in Oya Browser and sign in as you always do, CAPTCHA and second
factor included. There is nothing to save: changes sync to the persona as you
go, and once more when you quit. **Save profile** in the connection dialog
forces a sync and tells you how many sites the persona now holds.

A passkey prompt cannot be shown inside Oya Browser. When a site asks for one
(Google does, for accounts that have a passkey), it is answered as if you had
dismissed the prompt, and the site offers its other ways in: choose **Try
another way** and use your password.

## Copy between personas

Each persona keeps its own device; only the logins move.

- **Console:** open the target profile, and under **Move logins** choose the
  source in **Copy from profile…**, then **Copy**.
- **CLI:** `oya cookies copy <from-persona> <to-persona>`
- **SDK:** `await oya.personas.copyCookies(fromId, toId)`

One account seen from many devices is what sites flag as a bot farm. Copy a
login to a second persona when you mean to move the account, not to run it from
both at once.

## Export

- **Console:** open the profile, **Move logins**, **Export**. You get
  `<profile>-logins.json`.
- **CLI:** `oya cookies export <persona> --out logins.json`, or
  `--format playwright` for the shape Playwright takes.
- **SDK:** `await oya.personas.cookies(id)` or
  `await oya.personas.cookies(id, 'playwright')`
- **API:** `GET /api/pool/cookies?persona=<id>&format=json|playwright|netscape`.
  `netscape` answers a `cookies.txt` for curl, wget and yt-dlp.

```js
// Playwright, already signed in
const cookies = await oya.personas.cookies(personaId, 'playwright');
await context.addCookies(cookies);
```

An export holds live sessions. Treat the file like a password: the CLI writes it
readable by you alone, and viewer credentials cannot export at all.

## Import a file

Any JSON list of cookies with `name`, `value` and `domain` works, including
exports from this product, from Playwright (`context.cookies()`), and from
cookie-export browser extensions. A cookie missing one of those, or already
expired, is skipped and counted.

- **Console:** open the profile, **Move logins**, **Import**, choose the file.
- **CLI:** `oya cookies import <persona> logins.json`
- **SDK:** `await oya.personas.importCookies(id, cookies)`
- **API:** `PUT /api/pool/cookies?persona=<id>` with `{ "cookies": [...] }`, at
  most 20,000 per call.

The persona's browsers pick the cookies up on their next visit to each site.
Imports are written to the audit log.

## How sync decides who wins

The server stamps each cookie when it really changes. A browser takes the
server's copy of a cookie only when it does not have that cookie, or when the
server's copy changed after the browser last synced. A browser's own change
that has not reached the server yet is never overwritten, and waits for the
connection rather than being dropped. So a login made during a network blip, or
a moment before a redirect to a sibling host, is kept rather than replaced by
the older session it superseded.
