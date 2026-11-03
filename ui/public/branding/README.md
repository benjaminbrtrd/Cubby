# Cubby branding

Approved 3D cube with two aligned openings, exported from the Cubby 3D kit.
`cubby-1024.png` is the master for native icon generation. All sizes keep the
approved 3D artwork, including at 16–32 px.

`cubby-wordmark.svg` contains the approved Manrope ExtraBold lettering as paths.
The Logo component uses it as a CSS mask so the lettering follows the current
theme and contrast settings without loading a font. Manrope's license is in
`Manrope-OFL.txt`. The `on-dark` and `on-light` wordmarks and complete lockups are
also available as standalone SVG assets.

Generate native icons from the repository root:

```powershell
npm run tauri -- icon ui/public/branding/cubby-1024.png --output output/branding/tauri-icons
```

The native icons (window, taskbar, executable, installer) show the cube alone, without the dark
tile: they are generated from a cut-out of `cubby-1024.png` on a transparent square, not from the
master itself.

Copy the generated desktop icons into `src-tauri/icons`. The checked-in Windows
`icon.ico` contains 16, 20, 24, 32, 40, 48, 64, 96, 128 and 256 px images for the
window, executable, taskbar and installer. Keep these resolutions when replacing
it. The native icon paths and NSIS installer icon are set in `tauri.conf.json`.

Favicons and touch icons are linked from `ui/index.html`. The manifest and its
icons live together in this directory; its start URL points at the app root.
