# Syncing with upstream PicoLimbo

This fork tracks `Quozul/PicoLimbo`. The actual modifications are tiny (one file: [pico_limbo/src/wynn.rs](../pico_limbo/src/wynn.rs)) — most of the workspace is upstream code that we want to keep current to pick up new Minecraft protocol versions.

## When to sync

- A new Minecraft client version comes out and Wynncraft players upgrade. Upstream's job is to add the protocol bits; we just merge.
- A user reports they can't connect to `verify.wynnvets.org` because their client is too new. Almost always upstream-side.
- Routinely, every few months — easier to merge small than enormous.

## Process

```bash
# 0. From a clean working tree on master.
git status
git fetch
git checkout master
git pull

# 1. Add upstream as a remote (one-time).
git remote add upstream https://github.com/Quozul/PicoLimbo.git
git fetch upstream

# 2. Look at what's incoming.
git log --oneline ..upstream/master | head -50

# 3. Merge.
git merge upstream/master
```

If there's a release tag you specifically want (e.g. `v1.12.2+mc26.1.2`), merge the tag instead of `upstream/master` — slightly more deliberate:

```bash
git fetch upstream --tags
git merge v1.12.2+mc26.1.2
```

## Files where conflicts happen, and the rules

### `pico_limbo/Cargo.toml`
**Keep local:** the `uuid = { workspace = true }` line — `wynn.rs` depends on it. Upstream may add or remove unrelated deps; take theirs and re-add the local lines.
**Likely-local additions:** `reqwest`, `axum` if the two-way HTTP server ever returns from the `two-way-api-archive` branch.

### `Cargo.lock`
Take upstream's. `cargo` re-resolves on the next build and re-introduces the local additions. Don't try to merge `Cargo.lock` by hand — the format isn't designed for it.

### `pico_limbo/src/handlers/play/commands.rs`
This is the chat hook. The local change adds the `crate::wynn::on_incoming_chat(...)` call and the `[PicoLimbo] Code sent.` reply. Upstream may refactor the `ChatMessagePacket` handler — re-apply the two local additions on top of whatever they did.

The reply uses `parse_mini_message`; if upstream's text-component module renames, follow them.

### `pico_limbo/src/wynn.rs`
This file is **entirely local**. It will never appear in upstream conflicts unless someone moves things around in `pico_limbo/src/`. If it does appear, port the contents to wherever they ended up.

### `docker/Dockerfile`
Upstream's Dockerfile is the build context the vets-deploy stack uses. The `pico_libraries/` subtree was recently added upstream — make sure the Docker `COPY` lines include it (commit `ffc3e46` was a fix for this). When merging, double-check `COPY pico_libraries pico_libraries` (or equivalent) is present.

### Crates under `crates/` and `pico_libraries/`
Always take upstream wholesale. We deliberately do **not** modify these — every change creates a permanent merge conflict. If we ever need to change one, fork it into a new local crate rather than editing in place.

## After merging

```bash
# 1. Build clean.
cargo build --release -p pico_limbo

# 2. If new errors point at our wynn.rs (rare): port the change. Otherwise it's just cargo lock re-resolution noise on first build.

# 3. Run locally for a sanity check.
cargo run --release -p pico_limbo
# Connect with a recent MC client to localhost:25565, type a message, watch dazebot/temp-server for the forwarded chat-line.
```

The CI in `.github/workflows/` is upstream's; it doesn't run our integration scenarios. Local testing matters here.

## Known divergences worth keeping in mind

- Local `wynn.rs` exports `on_incoming_chat`. If upstream introduces a similarly-named hook in the future, we might be able to delete `wynn.rs` entirely and use theirs. Until then, the local code is what dazebot relies on.
- Two-way HTTP server (server-initiated `/send` messages to specific players) lives on the `two-way-api-archive` branch, removed from master. Restore from that branch if the use case returns; don't try to reimplement on master from scratch.
- The `[PicoLimbo] Code sent.` reply is a local add. Upstream has no opinion on chat replies — they treat chat as opaque.

## Don't

- **Don't squash-merge an upstream merge.** That throws away the upstream commit history and makes the next merge harder. Default merge commit (or `--no-ff`) is correct here.
- **Don't rebase our master onto upstream.** Same problem — re-writes history that's already pushed.
- **Don't take the upstream `Dockerfile` blindly without verifying our env vars work.** Upstream may add `ENV` directives we don't want, or remove ones we need. Diff before recreating production.
