# The macOS app

`Azeroth Survivors.app` is a small Swift launcher with the game beside it:

```
Azeroth Survivors.app/Contents/
  MacOS/AzerothSurvivors      the launcher (macos/Launcher/*.swift)
  MacOS/warcraft-survivors    the game: a player build (--no-default-features), ship profile
  Frameworks/Sparkle.framework
  Resources/                  icon, key art, data-manifest.tsv
```

The launcher:

- **Downloads the game data** on first run into `~/Library/Application Support/Azeroth Survivors/Data`.
  No Blizzard files ship in the app. It reads the archive.org zip's directory over HTTP Range
  requests and fetches each MPQ's own byte range, three at a time (archive.org paces each
  connection), so every file resumes on its own and only a missing or damaged file is fetched
  again. Each file is checked against `scripts/data-manifest.tsv` (SHA-256). The Dock icon shows
  the progress, and a notification says when it's done if the launcher isn't in front. Settings
  can point at an existing 1.12.1 `Data` folder instead.
- **Edits the game's settings** (graphics with Performance / Balanced / Quality presets, sound):
  the cvars in `benilla-config/config.toml`,
  kept in Application Support (`$BENILLA_HOME`), never inside the bundle. The game is only told
  what it already reads at boot; the launcher edits the file only while the game is closed.
- **Updates itself** with [Sparkle](https://sparkle-project.org) from the feed at
  `https://github.com/<repo>/releases/latest/download/appcast.xml`.
- Logs the game to `~/Library/Logs/Azeroth Survivors/game.log` and shows its tail on a crash.
- **Notices a stuck start:** if the game hasn't opened its window after 25 s (in practice an audio
  driver not answering, such as a remote-desktop one), the launcher comes back with Keep Waiting
  and Force Quit, and steps aside again by itself if the window appears.
  `defaults write io.github.matguitarist.azeroth-survivors stuckAfter -float 3` shortens the wait for testing.

## Building

```bash
make app            # → target/macos/Azeroth Survivors.app, signed with your Developer ID
make release-mac    # also notarize + staple, and write target/macos/dist/{zip,dmg,appcast.xml}
```

`scripts/macos-app.sh` holds the details and its environment knobs (`ARCHS="arm64 x86_64"` for a
universal build, `SIGN_IDENTITY`, `VERSION`, `BUILD`). The build number Sparkle compares comes
from the version (1.2.3 → 1002003), so it only ever grows with the version, whatever the history. Without a Developer ID certificate it
signs ad-hoc: the app runs on the Mac that built it, with updates off.

## One-time setup for releases

1. **Notarization credentials** for local releases, stored in the keychain (an app-specific
   password from appleid.apple.com):

   ```bash
   xcrun notarytool store-credentials AzerothSurvivors --apple-id <you@example.com> --team-id 434NVXAGUS
   ```

2. **The Sparkle signing key.** Its private half is in the login keychain under the account
   `azeroth-survivors`; the public half is `macos/sparkle-public-key.txt`, baked into every
   build. Back the private key up somewhere safe: an app signed with a lost key can never be
   updated again.

   ```bash
   target/macos/sparkle-2.10.0/bin/generate_keys --account azeroth-survivors -x sparkle-private-key.txt
   ```

3. **GitHub Actions secrets** for `.github/workflows/macos-release.yml`, all set by one script
   that asks for the passwords in hidden prompts:

   ```bash
   scripts/macos-ci-secrets.sh ~/Desktop/devide.p12
   ```

   | secret | what |
   |---|---|
   | `MACOS_CERT_P12`, `MACOS_CERT_PASSWORD` | the Developer ID Application certificate and key, exported from Keychain Access as .p12 |
   | `NOTARY_APPLE_ID`, `NOTARY_PASSWORD`, `NOTARY_TEAM_ID` | the Apple ID, an app-specific password for it, and the team |
   | `SPARKLE_PRIVATE_KEY` | the update signing key, exported from the keychain |

   An App Store Connect API key works for notarization instead: set `NOTARY_KEY_P8` (the .p8
   file's contents), `NOTARY_KEY_ID` and `NOTARY_ISSUER`.

## Shipping a version

1. Bump `version` in the workspace `Cargo.toml` and merge.
2. Tag and push: `git tag v0.2.1 && git push origin v0.2.1`. A tag with a suffix
   (`v0.2.1-ci1`) is a dry run: everything but publishing, the files kept as a workflow artifact.
3. The workflow builds a universal app, notarizes it, and publishes a release with the dmg,
   the zip and `appcast.xml`. Installed apps see the update on their next check.

A release can also be made locally with `make release-mac` and uploaded with
`gh release create v<version> target/macos/dist/*`. Only the latest non-draft, non-prerelease
release's `appcast.xml` is the live feed.
