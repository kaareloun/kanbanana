# Releasing Kanbanana

A release is one command:

```
scripts/release.sh 0.2.0
```

It bumps the version in `Cargo.toml`, commits `chore: release v0.2.0`, tags `v0.2.0` and
pushes. The tag starts `.github/workflows/release.yml`, which:

1. checks that the tag matches the version in `Cargo.toml`;
2. builds `kanbanana` for Linux x86_64, macOS Apple silicon and macOS Intel;
3. publishes the GitHub release with the archives, their checksums and generated notes;
4. writes `Formula/kanbanana.rb` (Homebrew), updates `packaging/aur/PKGBUILD` and
   `packaging/aur-bin/PKGBUILD`, and commits them to `main`;
5. pushes both AUR packages, with fresh checksums and `.SRCINFO`: `kanbanana` (builds
   from source) and `kanbanana-bin` (the prebuilt Linux archive from the release).

Follow it with `gh run watch`. Everything lives in this repo: the repo is its own Homebrew
tap, and the AUR packages are pushed from here.

## One-time setup

The AUR step needs an SSH key that may push to the AUR. Use a key just for this, not your
personal key, so the workflow can do nothing else with it:

```
ssh-keygen -t ed25519 -N "" -C "kanbanana-release@github-actions" -f /tmp/kanbanana-aur
gh secret set AUR_SSH_PRIVATE_KEY --repo kaareloun/kanbanana < /tmp/kanbanana-aur
cat /tmp/kanbanana-aur.pub
rm /tmp/kanbanana-aur /tmp/kanbanana-aur.pub
```

Paste the printed public key into <https://aur.archlinux.org> under My Account, SSH Public
Key (one key per line, next to your own), and save.

The first push of each name creates the `kanbanana` and `kanbanana-bin` packages on the
AUR under your account.

## How users install

- Arch: `yay -S kanbanana-bin` (prebuilt, fast) or `yay -S kanbanana` (builds from the
  release source).
- macOS and Linux with Homebrew:

  ```
  brew tap kaareloun/kanbanana https://github.com/kaareloun/kanbanana
  brew install kanbanana
  ```

  The formula uses the prebuilt archives from the GitHub release, so there is no Rust
  build on the user's machine. `brew upgrade` picks up new releases.
- Anyone: download an archive from the GitHub release page.

## If a step fails

- Tag and Cargo.toml differ: delete the tag (`git push --delete origin v0.2.0` and
  `git tag -d v0.2.0`), fix the version, run the script again.
- A build fails: fix it on `main`, then delete and push the tag again; the release is
  only created after all builds pass.
- The AUR step fails (key, network): re-run the failed job from the Actions page. The
  GitHub release and the Homebrew formula are already done at that point.

## Later

- A double-clickable macOS app (a cask with `Kanbanana.app`) needs an app bundle, and it
  opens without a warning only when it is signed and notarized with an Apple Developer
  account (99 USD a year).
- homebrew-core (plain `brew install kanbanana` without the tap) needs a project with some
  traction; apply when it has users.
