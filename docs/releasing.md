# Releasing

## Cutting a release

1. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
   `package.json` (keep all three equal). In `CHANGELOG.md`, move the
   `[Unreleased]` entries under a new `## [X.Y.Z] - YYYY-MM-DD` heading and
   add its link reference at the bottom. The release needs that section:
   `node scripts/changelog-section.mjs X.Y.Z` prints it, and fails when it is
   missing or empty.
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. `.github/workflows/release.yml` builds every platform and uploads the
   bundles to a **draft** GitHub release:

   | Runner | Target | Bundles |
   |---|---|---|
   | macos-latest | aarch64-apple-darwin | `mdview_X.Y.Z_aarch64.dmg` |
   | macos-latest | x86_64-apple-darwin | `mdview_X.Y.Z_x64.dmg` |
   | ubuntu-latest | x86_64-unknown-linux-gnu | `.deb`, `.rpm`, `.AppImage` |
   | windows-latest | x86_64-pc-windows-msvc | `.msi`, NSIS `.exe` |

   Every job first runs `scripts/changelog-section.mjs` against the tag, so a
   tag without a changelog section fails before building, and the section
   becomes the draft's description. The macOS jobs wait for approval in the
   `release` environment (see below).
4. Check the draft and publish it. Publishing is what Homebrew picks up.

## macOS signing and notarization

The workflow signs and notarizes the macOS app when these secrets are set in
the `release` GitHub Environment (Settings → Environments, with required
reviewers, so a tag push waits for approval before the macOS jobs get the
secrets). Without `APPLE_CERTIFICATE` it builds unsigned, with a warning.

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` password |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` | Apple ID email |
| `APPLE_PASSWORD` | app-specific password |
| `APPLE_TEAM_ID` | Team ID |

These are the same values aximar uses. GitHub never reveals a stored secret,
so set them again on this repository, for example
`base64 -i cert.p12 | gh secret set APPLE_CERTIFICATE --env release -R cmsd2/mdview`.

The Tauri CLI imports the certificate into a temporary keychain, signs with
the hardened runtime and staples the notarization ticket. mdview loads no
plugins or external dylibs and runs no other processes, so the hardened
runtime needs no extra entitlements. Unsigned builds trigger Gatekeeper
warnings on first launch.

## Homebrew

mdview ships through the `cmsd2/homebrew-tap` tap, alongside aximar:

```sh
brew install --cask cmsd2/tap/mdview
```

The cask lives in that repository (`Casks/mdview.rb`). Its `bump.py` rewrites
the version and both DMG checksums from the latest **published** (non-draft)
`v*` release of `cmsd2/mdview`, and its `Bump` workflow runs that hourly or on
demand. A draft release is invisible to it, so publishing the draft is the
Homebrew release step.
