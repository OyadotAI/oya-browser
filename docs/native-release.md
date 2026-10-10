# Native release acceptance

Oya's production browser control uses browser-owned APIs. External CDP clients
connect to an authenticated compatibility adapter that translates supported
commands; unsupported commands fail explicitly. The browser package cannot use a
stock Electron distribution as a fallback.

Run the production dependency checks and ordinary browser checks first:

```sh
cd browser
npm run test:unit
npm run lint
npm run format:check
npm run typecheck
npm run build
```

Supply the absolute path to the patched Oya executable, then run the native
acceptance matrix. It records each suite's output and an aggregate JSON report,
including platform and architecture. The suites run sequentially because desktop
focus is shared.

```sh
OYA_NATIVE_ENGINE=/absolute/path/to/Oya/engine \
OYA_ACCEPTANCE_OUTPUT=/absolute/path/to/results \
npm run test:native-release
```

The matrix covers pre-script policy before page/frame/popup and worker execution,
production app startup, persona policy and storage, private contexts, isolated V8
worlds, the external compatibility adapter, trusted input, cross-process frames,
drag, native choosers/dialogs, recording, workflow parity, analyzer and recorder.
Workflow select/file assignments expose synthetic events; they are not native
file chooser interactions. Chooser behavior has its own native fixture.

Packaging requires `OYA_NATIVE_DISTRIBUTIONS` containing target-specific unpacked
engines, for example `darwin-arm64/Electron.app`. The packaging hook executes
native API and first-script probes against that exact engine before unpacking.
A target must be built and tested on its own OS. A successful macOS arm64 run does
not establish macOS x64 or Windows support.

After signing and notarizing, test the actual packaged executable independently:

```sh
OYA_PACKAGED_EXECUTABLE='/absolute/path/to/Oya Browser.app/Contents/MacOS/Oya Browser' \
npm run test:native-packaged-app
```

This launches the shipped app with a disposable profile, opens a normal local
page, and checks authenticated native text input, trusted events, screenshot
capture and owned-target cleanup. It does not use a user's existing browser
profile. Publishing requires the final artifact's own passing test and checksum.
Apple's restricted browser credential entitlement additionally requires a valid
provisioning profile; ordinary Developer ID signing does not grant it.
