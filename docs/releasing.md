# Releasing

Releases are built, signed, and published by
[`.github/workflows/release.yml`](../.github/workflows/release.yml) when a
version tag is pushed:

1. Bump `version` in `Cargo.toml` (every commit does this anyway) and push.
2. Tag that commit with the same version and push the tag:

   ```
   git tag v0.0.20
   git push origin v0.0.20
   ```

The workflow builds and tests Windows and macOS, runs the self-update
end-to-end test on both, checks the tag matches `Cargo.toml`, signs the
`.exe` and the macOS `.zip`, and publishes them with their `.minisig`
signatures. Copies of the game from v0.0.20 on offer the new release at
startup (see `src/update.rs`).

## The signing key

The game only installs updates signed by the minisign key whose public half
is [`keys/release-signing.pub`](../keys/release-signing.pub) (built into the
game). The secret half:

- lives outside the repo at `~/.config/test-your-might/release-signing.key`
  on the machine it was made on (never commit it; `*.key` is gitignored);
- must be stored in the GitHub repository secret `MINISIGN_SECRET_KEY`
  (Settings > Secrets and variables > Actions > New repository secret), with
  the whole file as the value, so the workflow can sign. Without it the
  publish job fails rather than publishing unsigned builds.

Keep a backup of the secret key somewhere safe (e.g. a password manager). If
it's lost, new releases can't be signed with it, and copies of the game in
people's hands will refuse every future update: they'd have to download a
build with a new key by hand once.

If the secret key leaks, anyone could sign an "update". Make a new key pair
(`minisign -G -W -p keys/release-signing.pub -s <new secret key>`), replace
the GitHub secret, and release; again, existing copies need one manual
download to pick up the new key.

Signing by hand (what the workflow runs, needs the minisign CLI):

```
tools/sign-release.sh <secret key file> <version> <file>...
```
