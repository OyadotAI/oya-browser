# Oya: send logins (Chrome and Firefox extension)

Sends the logins (cookies) for the site you are on to one of your Oya profiles, so Oya browsers on that profile
start signed in. One Manifest V3 source in `src/`, plain JavaScript with no build step, shipped to both stores.

- Asks for no site at install. Pressing **Send** asks for the current site only (`*://*.<site>/*`, so logins kept
  on the parent domain are readable), then reads that site's cookies and sends them in one
  `PUT https://oyabrowser.com/api/pool/cookies?persona=<id>`.
- The API key is kept in `storage.local` and sent only to oyabrowser.com. A viewer key is refused before any cookie
  is read. Nothing runs in the background.

## Build the store uploads

```sh
npm run pack --workspace=@oya-ai/extension   # dist/oya-chrome.zip, dist/oya-firefox.zip
npx web-ext lint -s packages/extension/dist/firefox
```

The version comes from `package.json`. The two manifests differ only by Firefox's `browser_specific_settings`.

## Try it before submitting

- Chrome: `chrome://extensions`, Developer mode, **Load unpacked**, pick `dist/chrome`.
- Firefox: `about:debugging#/runtime/this-firefox`, **Load Temporary Add-on**, pick `dist/firefox/manifest.json`.

Sign in to a site, open the popup, add a key in Settings, press Send, then open the site in an Oya browser on that
profile: it should be signed in.

## Store listing

- **Name:** Oya: send logins
- **Short description:** Send the logins for the site you are on to an Oya profile, so your Oya browsers start
  signed in.
- **Description:** Signed in to a site here and want your Oya browsers signed in too? Open this extension, pick an
  Oya profile and press Send. It asks to read the current site only, at the moment you press Send, and sends that
  site's cookies straight to your Oya account over HTTPS. Nothing runs in the background, nothing is sent anywhere
  else, and your API key stays in this browser. Some banks and sites that tie a session to one device may ask you
  to sign in once more in Oya.
- **Category:** Developer tools (Chrome), Privacy and security or Other (Firefox)
- **Privacy policy:** https://oyabrowser.com/privacy
- **Homepage / support:** https://oyabrowser.com
- **Icon:** `src/icon.png` (128x128)
- **Chrome permission justifications:**
  - `cookies`: reads the current site's cookies when the person presses Send, to copy their login to Oya.
  - `storage`: keeps the person's Oya API key and last profile in this browser.
  - `activeTab`: reads the address of the tab the popup opened over.
  - host access to `https://oyabrowser.com/*`: the only server the extension talks to, for the profile list and
    the import.
  - optional host access: asked for one site at a time, when the person presses Send.
- **Chrome data usage:** collects authentication information (cookies), sent only to the person's own Oya
  account; not sold, not used for anything unrelated, not used for creditworthiness.
- **Firefox:** add-on id `extension@oyabrowser.com`; declares `authenticationInfo` data collection. The upload is
  the source, so no separate source archive is needed.
