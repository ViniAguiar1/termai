# Releasing termAI

Distribution has three moving parts:

1. **GitHub Releases** — the source of truth. Each version is a `vX.Y.Z` tag with
   a notarized DMG attached.
2. **Homebrew tap** — `ViniAguiar1/homebrew-termai`, so users can
   `brew install --cask termai` and `brew upgrade --cask termai`.
3. **In-app update check** — the Go engine queries the GitHub Releases API on
   startup and, if a newer tag exists, the terminal shows an overlay with a
   "open download page" action.

## Versioning (single source of truth)

The version comes from the git tag (`vX.Y.Z`). `packaging/macos/package.sh`
derives it as: explicit arg → latest `v*` tag → `Cargo.toml` version, and stamps
it into the DMG name, the Go binary (`-X ...cmd.appVersion`), and `Info.plist`.

Before tagging a release, bump `version` in the root `Cargo.toml` to match, so
the Rust binary's `CARGO_PKG_VERSION` (used by the in-app update check) lines up
with the tag.

## Cutting a release

```bash
# 1. Bump Cargo.toml version to X.Y.Z, commit.
# 2. Tag and push:
git tag vX.Y.Z
git push origin vX.Y.Z
```

The `.github/workflows/release.yml` workflow then:

1. Builds Go + Rust release binaries.
2. Imports the signing cert, signs and notarizes, and runs `package.sh`.
3. Creates the GitHub Release and uploads the DMG.
4. Stamps `packaging/homebrew/termai.rb` with the version + DMG sha256 and pushes
   it to the tap as `Casks/termai.rb`.

### Required repository secrets

| Secret | What |
| --- | --- |
| `MACOS_CERT_P12_BASE64` | base64 of the "Developer ID Application" cert (`.p12`) |
| `MACOS_CERT_PASSWORD` | password for that `.p12` |
| `APPLE_SIGN_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_TEAM_ID` | Apple Developer team id |
| `APPLE_ID` | Apple account email (notarization) |
| `APPLE_APP_PASSWORD` | app-specific password for `notarytool` |
| `HOMEBREW_TAP_TOKEN` | PAT with write access to `homebrew-termai` |

Export the cert from Keychain Access (Developer ID Application → Export → `.p12`),
then `base64 -i cert.p12 | pbcopy` to get `MACOS_CERT_P12_BASE64`.

## One-time tap setup

```bash
gh repo create ViniAguiar1/homebrew-termai --public \
  --description "Homebrew tap for termAI"
git clone https://github.com/ViniAguiar1/homebrew-termai.git
mkdir -p homebrew-termai/Casks
cp packaging/homebrew/termai.rb homebrew-termai/Casks/termai.rb
# (set a real sha256 or leave the release workflow to bump it)
cd homebrew-termai && git add . && git commit -m "init cask" && git push
```

After the first release, users install with:

```bash
brew tap viniaguiar1/termai
brew install --cask termai
```

## Local builds (unsigned / ad-hoc)

For local testing without Apple credentials, build the binaries and assemble an
ad-hoc-signed bundle manually (see `packaging/macos/package.sh` for the bundle
layout). `package.sh` itself requires the signing/notarization env vars.
