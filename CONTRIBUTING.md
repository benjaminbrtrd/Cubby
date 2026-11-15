# Contributing to Cubby

Thanks for helping. Here's how to get going.

## Reporting a bug

Open an issue with:

- what you did and what happened,
- your Cubby version and Windows version,
- for a game crash, the report from the crash screen (the **Copy report** button).

## Making a change

1. Build the app as described in the [README](README.md#building-from-source).
2. Keep a pull request to one change. If it's a big one, open an issue first so we can agree on the approach.
3. Before you push, run:

   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets
   cargo test --workspace
   cd ui && npm run check && npm test
   ```

## Code

- Logic goes in the `crates/`. `src-tauri` only wires it to the interface.
- A crate depends on `cubby-common` and nothing else from the workspace, so each one builds and tests on its own.
- Tests that call real services are marked `#[ignore]`, so `cargo test` works offline.
- Comments explain why, not what. If the code needs a paragraph to be understood, it can probably be simpler.

## Interface text

The interface is in English for now. Write it the way the app already speaks:

- Short sentences, sentence case, no exclamation marks.
- Buttons start with a verb: *Play*, *Install*, *Move to trash*.
- An error says what happened and what to do about it.
- Use the game's words: *instance*, *loader*, *mods*, *resource packs*, *shader packs*, *worlds*.

## Releasing

1. Set the version in `Cargo.toml`, `src-tauri/tauri.conf.json` and `ui/package.json`.
2. Write the notes in `.github/releases/v<version>.md`, with a screenshot of what's new.
3. Push a tag such as `v2026.11.0`. GitHub Actions builds a draft release.

### Signing the macOS app

The release workflow signs the macOS builds with a Developer ID certificate and notarizes them, when these repository secrets exist (without them the builds are unsigned and macOS warns on first launch):

| Secret | What it holds |
| --- | --- |
| `APPLE_CERTIFICATE` | The *Developer ID Application* certificate and its private key, exported from Keychain Access as a `.p12`, in base64 (`base64 -i cert.p12 \| pbcopy`). |
| `APPLE_CERTIFICATE_PASSWORD` | The password chosen when exporting the `.p12`. |
| `APPLE_SIGNING_IDENTITY` | The certificate's name, e.g. `Developer ID Application: Jane Doe (AB12CD34EF)` (`security find-identity -v -p codesigning`). |
| `APPLE_API_ISSUER` | App Store Connect › Users and Access › Integrations › App Store Connect API: the *Issuer ID*. |
| `APPLE_API_KEY` | The *Key ID* of a Team key with the Developer role. |
| `APPLE_API_PRIVATE_KEY` | The content of that key's `AuthKey_<Key ID>.p8` file (it can be downloaded only once). |

The first three sign; the last three notarize. Check a signed build with `spctl -a -vv /Applications/Cubby.app`: it should say *accepted* and *source=Notarized Developer ID*.

## License

By contributing, you agree that your work is released under the [GPL-3.0](LICENSE), like the rest of Cubby.
