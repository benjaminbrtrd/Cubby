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

## License

By contributing, you agree that your work is released under the [GPL-3.0](LICENSE), like the rest of Cubby.
