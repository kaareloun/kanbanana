# Releasing Kanbanana

Three channels: a GitHub release with prebuilt archives, the AUR for Arch, and a Homebrew
tap for macOS (and Linux). Do them in this order, because the AUR package and the Homebrew
formula both download the source archive of the GitHub tag.

## 1. GitHub release

1. Set the version in `Cargo.toml` (`[workspace.package] version`), run
   `cargo build -p kanbanana` once so `Cargo.lock` updates, and commit:
   `git commit -am "chore: release v0.1.0"`.
2. Tag and push:

   ```
   git tag v0.1.0
   git push origin main v0.1.0
   ```

3. `.github/workflows/release.yml` builds `kanbanana` for Linux x86_64, macOS Apple
   silicon and macOS Intel, and attaches the archives and their `.sha256` files to the
   release `v0.1.0`. Watch it with `gh run watch`.
4. Edit the release notes on GitHub if you like (`gh release edit v0.1.0 --notes-file ...`).

## 2. AUR (Arch Linux)

One time:

1. Create an account on <https://aur.archlinux.org> and add your SSH public key in
   "My Account".
2. Check that the name is still free: <https://aur.archlinux.org/packages/kanbanana>.

Each release:

1. Get the checksum of the tag archive:

   ```
   curl -sL https://github.com/kaareloun/kanbanana/archive/refs/tags/v0.1.0.tar.gz | sha256sum
   ```

2. Clone the AUR repo (the first clone of a new name is empty, that is fine):

   ```
   git clone ssh://aur@aur.archlinux.org/kanbanana.git ~/aur/kanbanana
   cp packaging/aur/PKGBUILD ~/aur/kanbanana/
   cd ~/aur/kanbanana
   ```

3. In `PKGBUILD` set `pkgver` to the new version, `pkgrel=1`, and replace `'SKIP'` in
   `sha256sums` with the checksum from step 1.
4. Build and test it in a clean environment, then write `.SRCINFO`:

   ```
   makepkg -si          # builds, runs the tests and installs it
   namcap PKGBUILD      # optional lint (pacman -S namcap)
   makepkg --printsrcinfo > .SRCINFO
   ```

5. Publish:

   ```
   git add PKGBUILD .SRCINFO
   git commit -m "Update to 0.1.0"
   git push
   ```

Users then install with `yay -S kanbanana` or `paru -S kanbanana`. Optionally, a second
package `kanbanana-bin` can use the prebuilt Linux archive from the GitHub release, so
users skip the Rust build.

## 3. Homebrew (macOS)

Homebrew's default place for small new projects is a tap: your own repo named
`homebrew-kanbanana`. Getting into homebrew-core itself needs a project with some
traction (stars, users), so start with the tap and apply later.

One time:

```
gh repo create kaareloun/homebrew-kanbanana --public --description "Homebrew tap for Kanbanana"
git clone https://github.com/kaareloun/homebrew-kanbanana ~/homebrew-kanbanana
mkdir -p ~/homebrew-kanbanana/Formula
```

Each release:

1. Get the checksum of the tag archive (same command as for the AUR).
2. Copy `packaging/homebrew/kanbanana.rb` to `~/homebrew-kanbanana/Formula/kanbanana.rb`,
   set the version in `url` and paste the checksum into `sha256`.
3. Test it on a Mac:

   ```
   brew install --build-from-source ~/homebrew-kanbanana/Formula/kanbanana.rb
   brew test kanbanana
   brew audit --strict --new kanbanana
   ```

4. Commit and push the tap:

   ```
   cd ~/homebrew-kanbanana
   git add Formula/kanbanana.rb
   git commit -m "kanbanana 0.1.0"
   git push
   ```

Users then install with:

```
brew install kaareloun/kanbanana/kanbanana
```

The formula builds from source and installs the `kanbanana` command. Start it from the
terminal with `kanbanana`. A double-clickable macOS app (a cask with `Kanbanana.app`)
needs an app bundle, and to open without a warning it must be signed and notarized with
an Apple Developer account (99 USD a year). That is a later step.

## Checklist

- [ ] CI is green on Linux and macOS (`.github/workflows/ci.yml`).
- [ ] Version bumped and tagged, release workflow green, archives on the release page.
- [ ] AUR: `PKGBUILD` version and checksum, `.SRCINFO` regenerated, pushed.
- [ ] Homebrew tap: formula version and checksum, `brew test` passes, pushed.
- [ ] README install section says how to install from the AUR and Homebrew.
