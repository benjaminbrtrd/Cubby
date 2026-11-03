# Cubby interface

The Cubby interface, in Svelte 5 and TypeScript, built with Vite.

```sh
npm install
npm run dev      # in a browser, with sample data
npm run check    # type check
npm run build    # into dist/, used by the desktop app
```

Inside the desktop app, `api.ts` sends every call to the Rust side through Tauri. In a plain browser there's no Rust side, so the same calls go to `lib/dev/mock.ts`, which answers with sample instances and mods. Add `?onboarding` to the address to see the first-launch screens.

- `lib/cubby/`: the design-system components (buttons, tiles, the hotbar…).
- `lib/views/`: one folder per screen.
- `lib/state.svelte.ts`: the app state shared by the screens.

To run it inside the app, see the [main README](../README.md#building-from-source).
