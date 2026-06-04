# Upgrading the upstream PicoLimbo ref

The repo is a patch overlay, not a fork. There is no `git merge upstream/master` workflow — instead, we bump a build-time ref and regenerate any patches that no longer apply.

## When to upgrade

- A new Minecraft client version comes out and Wynncraft players upgrade. Upstream ships protocol bits as releases tagged `vX.Y.Z+mcA.B.C`.
- The `upstream-smoke` workflow goes red — upstream made a change that breaks our patches or compile.
- Periodically, every few months, just to stay current.

## How to upgrade

1. Pick a new upstream ref (tag preferred):
   ```bash
   gh release list --repo Quozul/PicoLimbo
   ```
2. Edit `PICOLIMBO_REF` default in [docker/Dockerfile](../docker/Dockerfile):
   ```diff
   -ARG PICOLIMBO_REF=v1.12.2+mc26.1.2
   +ARG PICOLIMBO_REF=v1.13.0+mc26.2.0
   ```
3. Test locally:
   ```bash
   docker build -t picolimbo:test -f docker/Dockerfile .
   ```
4. If the build succeeds → open a PR, `verify-patches` CI gates it, merge. On merge the same workflow pushes the new `:<sanitized-ref>` image to GHCR (OCI tags can't contain `+`, so `v1.13.0+mc26.2.0` → `:v1.13.0-mc26.2.0`).
5. Bump `IMAGE_TAG` in `vets-deploy/stacks/picolimbo/.env` to the sanitized form, commit, push. On the VPS: `manage sync && manage update auth-stack`.

## When `git apply` fails

The build's `source` stage will print exactly which patch and which file
failed. Regenerate the patch:

```bash
# Get a fresh upstream tree at the target ref
git clone https://github.com/Quozul/PicoLimbo.git /tmp/pl
cd /tmp/pl
git checkout v1.13.0+mc26.2.0

# Try to apply the existing patches; resolve conflicts manually
git apply --reject /path/to/auth-stack/patches/000X-foo.patch
# Look for *.rej files, integrate the rejected hunks by hand,
# delete the *.rej files when done.

# Stage the result and re-export
git add -u
git diff --cached > /path/to/auth-stack/patches/000X-foo.patch
```

Commit the regenerated patch, push, let CI verify.

## When the patched source no longer compiles

Upstream changed an API our patches depend on (e.g. renamed a function in
`pico_text_component` that `commands.rs` calls). Same regeneration loop, but
you'll need to edit the patched files to match the new API before
re-exporting.

## Files our patches touch (predictable conflict surface)

| File | Why it might conflict |
|------|------------------------|
| `pico_limbo/src/handlers/play/commands.rs` | Upstream may refactor the `ChatMessagePacket` handler. The patch adds an `on_incoming_chat` call and a "Code sent" reply in the non-command branch — re-thread those onto whatever upstream did. |
| `pico_limbo/src/lib.rs` | Single `pub mod wynn;` line. Conflicts only if upstream reorders module declarations. |
| `pico_limbo/src/main.rs` | Same. |
| `pico_limbo/src/wynn.rs` | Pure addition, no upstream version. Conflicts only if upstream creates a file at the same path (extremely unlikely). |

If the patches' surface area grows, that's a signal to think about whether the
addition is structural enough to upstream. (The natural target would be a
generic "chat webhook" feature in `server.toml`, which would let us delete the
overlay entirely and run `ghcr.io/quozul/picolimbo` directly.)

## Don't

- **Don't add upstream code into the repo to "make patching easier."** That defeats the structural purpose. Edit patches by cloning upstream into a scratch dir.
- **Don't pin to `master` in production.** `PICOLIMBO_REF` should be a tag in production. The nightly smoke build is what runs against `master`.
- **Don't skip `verify-patches` CI.** A red CI on master is what would silently break the next deploy.
