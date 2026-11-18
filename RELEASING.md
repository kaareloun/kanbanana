# Releasing Kanbanana

A release is one command:

```
scripts/release.sh 0.2.0
```

It bumps the version in `Cargo.toml`, commits `chore: release v0.2.0`, tags `v0.2.0` and
pushes. CI skips that commit; the release workflow tests it instead. The tag starts
`.github/workflows/release.yml`, which checks that the tag matches `Cargo.toml` and then
runs two independent tracks:

- **Linux** (a few minutes): run the tests, build Linux x86_64, create the GitHub release
  with the Linux archive, push both AUR packages with fresh checksums and `.SRCINFO`
  (`kanbanana` builds from source, `kanbanana-bin` uses the prebuilt archive), and commit
  the PKGBUILDs to `main`.
- **macOS** (whenever a macOS runner is free): build Apple silicon and Intel in one job,
  add both archives to the same release, write `Formula/kanbanana.rb` and commit it to
  `main`.

Free macOS runners can queue for a long time. The Linux track never waits for them, so
Arch users get the release first.

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
- A build or test fails: fix it on `main`, then delete and push the tag again. Nothing is
  published before the Linux build and tests pass.
- The macOS job is cancelled or fails: re-run only that job (and `homebrew` after it)
  from the Actions page. The Linux release and the AUR packages are already out.
- The AUR step fails (key, network): re-run the `aur` job from the Actions page.

## Later

- A double-clickable macOS app (a cask with `Kanbanana.app`) needs an app bundle, and it
  opens without a warning only when it is signed and notarized with an Apple Developer
  account (99 USD a year).
- homebrew-core (plain `brew install kanbanana` without the tap) needs a project with some
  traction; apply when it has users.
