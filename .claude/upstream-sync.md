# Investigating a red `upstream-track` workflow

The repo is a patch overlay, not a fork. There is no `git merge upstream/master` workflow — instead, the `upstream-track` GitHub Action queries Quozul/PicoLimbo's latest release tag on every push/schedule, applies our patches, and rebuilds. When the build goes red, an upstream change has broken our patches or compile. This doc is the recovery playbook.

## What "auto" means in practice

- `upstream-track` runs on master push and daily at 06:00 UTC.
- On success → publishes `ghcr.io/wynncraft-veterans/auth-stack:<sanitized-ref>` AND `:rolling`.
- On failure → publishes nothing. `:rolling` keeps its previous digest. Production keeps running on the previous build.
- Production unblocks itself the moment a subsequent run goes green; you don't have to rush the fix.

If `:rolling` shipped a runtime-bad image before the fault was caught, the operator-side recovery is:
```bash
manage pin auth-stack <last-known-good-sanitized-tag>
```
(See [vets-deploy/stacks/picolimbo/README.md](../../vets-deploy/stacks/picolimbo/README.md).)

## Triage flow when a run is red

1. Open the failed run in `Actions → upstream-track`. Read the "Resolve target upstream ref" step — that tells you which upstream tag the workflow tried.
2. Scroll the build log:
   - `git apply` failed → patches don't apply, jump to **"When `git apply` fails"** below.
   - `cargo build` failed → upstream API change, jump to **"When the patched source no longer compiles"**.
   - Rust toolchain failure → upstream bumped `rust-toolchain.toml` past what `rust:1-alpine` ships; not common, but see **"Toolchain drift"**.
3. Reproduce locally before editing patches:
   ```bash
   docker build --build-arg PICOLIMBO_REF=<failing-tag> \
                -t picolimbo:debug -f docker/Dockerfile .
   ```

## When `git apply` fails

Regenerate the affected patch against the failing upstream tag:

```bash
# Fresh upstream tree at the target ref
git clone https://github.com/Quozul/PicoLimbo.git /tmp/pl
cd /tmp/pl
git checkout <failing-tag>

# Try the existing patch; resolve rejected hunks by hand
git apply --reject /path/to/auth-stack/patches/000X-foo.patch
# integrate any *.rej hunks into the working tree, delete .rej files

# Stage and re-export
git add -u
git diff --cached > /path/to/auth-stack/patches/000X-foo.patch
```

Commit the regenerated patch and push. CI rebuilds; on green, `:rolling` re-tags.

## When the patched source no longer compiles

Upstream changed an API our patches depend on (e.g. renamed a function in `pico_text_component` that `commands.rs` calls). Same regeneration loop, but you'll also need to edit the patched files to match the new API before re-exporting.

## Toolchain drift

The Dockerfile uses `rust:1-alpine`, which floats with stable 1.x. Upstream's `rust-toolchain.toml` takes precedence via rustup. If upstream pins a nightly or a 1.x newer than the alpine image ships, builds fail at the toolchain step. Options:
- Pin the alpine image to a newer tag if available (`rust:1.96-alpine` etc.).
- Switch to `rust:1` (Debian) temporarily — costs ~80MB on the builder stage but is fine for a stopgap.

## Files our patches touch (predictable conflict surface)

| File | Why it might conflict |
|------|------------------------|
| `pico_limbo/src/handlers/play/commands.rs` | Upstream may refactor the `ChatMessagePacket` handler. The patch adds an `on_incoming_chat` call and a "Code sent" reply in the non-command branch — re-thread those onto whatever upstream did. |
| `pico_limbo/src/lib.rs` | Single `pub mod wynn;` line. Conflicts only if upstream reorders module declarations. |
| `pico_limbo/src/main.rs` | Same. |
| `pico_limbo/src/wynn.rs` | Pure addition, no upstream version. Conflicts only if upstream creates a file at the same path (extremely unlikely). |

If the patches' surface area grows, that's a signal to think about whether the addition is structural enough to upstream. (The natural target would be a generic "chat webhook" feature in `server.toml`, which would let us delete the overlay entirely and run `ghcr.io/quozul/picolimbo` directly.)

## Forcing an out-of-band build against a specific ref

`Actions → upstream-track → Run workflow → ref=<tag>`. Pushes the same tag pair as a scheduled run. Useful for:
- Testing whether a specific upstream tag will build green before letting the daily schedule discover it.
- Rebuilding the current `:rolling` against a different upstream (e.g. rolling back the API-resolved target without editing the Dockerfile).

## Don't

- **Don't add upstream code into the repo to "make patching easier."** That defeats the structural purpose. Edit patches by cloning upstream into a scratch dir.
- **Don't pin to `master` in `PICOLIMBO_REF`.** The workflow targets release tags by design; `master` can include WIP that breaks our patches between releases.
- **Don't hand-publish images to GHCR.** Trigger `workflow_dispatch` instead — the workflow is the only thing that re-tags `:rolling`, and a manual `docker push` outside CI will silently desync `:rolling` from `:<sanitized-ref>`.
