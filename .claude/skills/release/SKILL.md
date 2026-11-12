---
name: release
description: Cut an mdview release - bump the version, cut the CHANGELOG section, tag v<version>, then see the signed GitHub release through to publishing and the Homebrew cask update. Use when the user asks to release, tag, ship or publish a new mdview version.
argument-hint: "<new-version>"
---

# Release mdview

A `v*` tag runs `.github/workflows/release.yml`, which builds every platform,
signs and notarizes the macOS app, and uploads everything to a **draft** GitHub
release whose description is the version's `CHANGELOG.md` section. Publishing
the draft is what `cmsd2/homebrew-tap` picks up. `docs/releasing.md` has the
background; this skill is the checklist.

`$ARGUMENTS` is the new version as plain `X.Y.Z` (no `v`, no `-rc`: the tap's
`bump.py` ignores anything else). If it is missing, read the current version
and the `[Unreleased]` entries, suggest the next version (minor for features,
patch for fixes only), and ask.

Run every command from the mdview repository root. The tag in particular: the
user's own terminal may sit in another repository (aximar also has `v*` tags).

## 1. Pre-flight checks

Stop and report if any check fails.

1. `git remote get-url origin` is `https://github.com/cmsd2/mdview.git`.
2. On `master`, working tree clean, and level with `origin/master` after
   `git fetch` (`git status -sb` shows no ahead/behind).
3. Neither `git tag -l vX.Y.Z` nor `git ls-remote --tags origin vX.Y.Z` finds
   the tag.
4. The `[Unreleased]` section of `CHANGELOG.md` has entries.
5. `gh secret list --env release -R cmsd2/mdview` lists all six of
   `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
   `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID`. Without them the macOS
   build ships unsigned: warn, and continue only if the user agrees.

## 2. Bump the version

All four must agree:

- `npm version X.Y.Z --no-git-tag-version` (updates `package.json` and
  `package-lock.json`)
- `"version"` in `src-tauri/tauri.conf.json`
- `[package] version` in `src-tauri/Cargo.toml`
- then `cargo check --manifest-path src-tauri/Cargo.toml` to update
  `src-tauri/Cargo.lock`

Confirm with `grep -n '"version"' package.json src-tauri/tauri.conf.json`,
`grep -n '^version' src-tauri/Cargo.toml` and the `name = "mdview"` entry in
`src-tauri/Cargo.lock`.

## 3. Cut the CHANGELOG section

- Rename the `[Unreleased]` entries to `## [X.Y.Z] - YYYY-MM-DD` (today) and put
  a fresh, empty `## [Unreleased]` above it.
- Link references at the bottom:
  `[Unreleased]: https://github.com/cmsd2/mdview/compare/vX.Y.Z...HEAD` and
  `[X.Y.Z]: https://github.com/cmsd2/mdview/compare/vPREV...vX.Y.Z`, where
  `vPREV` is the previous tag on GitHub. Use
  `https://github.com/cmsd2/mdview/releases/tag/vX.Y.Z` when there is none
  (`v0.1.0` was never tagged).
- Compare the entries with `git log <previous tag or root>..HEAD --oneline` and
  propose any user-facing change that is missing.
- `node scripts/changelog-section.mjs X.Y.Z` must print the section. CI runs the
  same script and fails every job if it doesn't.
- Show the user the section: it becomes the public release notes.

## 4. Check

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `node --check frontend/app.js`

## 5. Commit, tag and push

- Stage the four version files and `CHANGELOG.md`, and commit as
  `Release vX.Y.Z`.
- Ask for confirmation, then
  `git tag vX.Y.Z && git push origin master && git push origin vX.Y.Z`.

## 6. See the release through

1. `gh run list -R cmsd2/mdview --workflow release.yml -L 1` finds the run. The
   two macOS jobs wait for the user's approval in the `release` environment;
   tell them, with the run's URL. Wait for the run's completion notification
   rather than polling it.
2. On failure, read the failing job's log (`gh run view <id> --log-failed`). A
   signing or notarization error usually means a wrong secret. To retry after a
   fix, delete the draft and the tag first:
   `gh release delete vX.Y.Z -R cmsd2/mdview --yes`,
   `git push --delete origin vX.Y.Z`, `git tag -d vX.Y.Z`. Then fix, commit,
   and tag again.
3. On success, `gh release view vX.Y.Z -R cmsd2/mdview` shows a draft with
   seven assets: `mdview_X.Y.Z_aarch64.dmg`, `mdview_X.Y.Z_x64.dmg`, `.deb`,
   `.rpm`, `.AppImage`, `.msi` and the NSIS `.exe`. Its description should be
   the CHANGELOG section.
4. Optional signing check on macOS: download the arm DMG, mount it, then
   `spctl -a -vv <mount>/mdview.app` should report `source=Notarized Developer
   ID`, and `xcrun stapler validate <mount>/mdview.app` should succeed.
5. Publish only when the user says so:
   `gh release edit vX.Y.Z -R cmsd2/mdview --draft=false`.

## 7. Homebrew

The tap's `Bump` workflow runs `bump.py` hourly. To run it now:
`gh workflow run Bump -R cmsd2/homebrew-tap`. Then confirm `Casks/mdview.rb` on
`cmsd2/homebrew-tap` main has `version "X.Y.Z"` and real checksums, and the
user can `brew upgrade --cask cmsd2/tap/mdview`.

## Summary

Report the old and new versions, the tag, the workflow run's URL and result,
whether the draft is published, and whether the cask has the new version.
