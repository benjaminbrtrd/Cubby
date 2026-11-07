# Winget

`winget install Benjaminbrtrd.Cubby` installs Cubby from the Windows Package Manager. The manifests in
`manifests/` describe one version of the Windows installer; they are what the first submission to
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) is made of. They were written by hand and
not checked with `winget validate`: run it before the pull request (on Windows: `winget validate --manifest <folder>`).

## First submission (once)

1. Pick the version to submit, ideally the first stable one (a pre-release is accepted but not what people expect).
2. Update `PackageVersion`, `InstallerUrl` and `InstallerSha256` in the three files (the sums are in the release's
   `SHA256SUMS.txt`), and the folder name.
3. Copy the folder to `manifests/b/Benjaminbrtrd/Cubby/<version>/` in a fork of `microsoft/winget-pkgs`, run
   `winget validate`, and open a pull request. Microsoft's bots test the installer; a person approves it.
   [Komac](https://github.com/russellbanks/Komac) (`komac new`) can write the files and the pull request for you.

## Every release after that

`.github/workflows/winget.yml` opens the pull request by itself when a non pre-release version is published.
Turn it on with the repository secret `WINGET_TOKEN` (classic token, `public_repo` scope, from an account with
a fork of `microsoft/winget-pkgs`) and the repository variable `WINGET_ENABLED=true`.

## Signing

Winget checks the SHA-256 of the installer, not its signature, so it works unsigned. When the Windows installer
is signed, the hash changes: always take it from the file that is published.
