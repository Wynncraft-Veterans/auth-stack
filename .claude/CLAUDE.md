# auth-stack

Patch overlay over [Quozul/PicoLimbo](https://github.com/Quozul/PicoLimbo) (Rust ultra-light Minecraft limbo server) that adds chat-line forwarding to dazebot for Minecraft account-link verification.

## Key facts

- **Production address:** `verify.wynnvets.org:25565` (Minecraft TCP)
- **Shape:** Two patches under [patches/](../patches/) applied to a fresh clone of `Quozul/PicoLimbo` at build time. No upstream code lives in this repo.
- **Build:** [docker/Dockerfile](../docker/Dockerfile) clones upstream at `PICOLIMBO_REF` (default = pinned tag), applies patches, builds the `pico_limbo` binary into a distroless image.
- **Why this overlay exists:** Upstream has no chat-forwarding mechanism; dazebot's link flow needs every chat line GET'd to its `/api/auth` endpoint.

## Related repos (same workspace)

- `../dazebot` — Receives the chat-line forwards at `GET /api/auth/{uuid}/{msg}` and looks for `LinkCode` matches. **The only consumer of auth-stack's HTTP forwarding.**
- `../vets-deploy` — Compose stack at `stacks/picolimbo/` pulls `ghcr.io/wynncraft-veterans/auth-stack:<sanitized-ref>` (no longer builds locally; auth-stack CI publishes the image).
- `../vetsmod` — Indirect: vetsmod users are Discord-linked via the auth-stack→dazebot chain *before* they ever run `/vetsmod` to get a vetsmod auth key. Two distinct auth flows that happen to share dazebot's `/api/auth/` HTTP prefix.
- `../temporary-server` — Unrelated.

## Where the modification lives

| Patch | What it does |
|-------|--------------|
| [patches/0001-add-wynn-chat-forward-module.patch](../patches/0001-add-wynn-chat-forward-module.patch) | Creates `pico_limbo/src/wynn.rs` with `REMOTE_API_URL` env var (default `http://localhost:9421/api/auth`) and the helper that GETs `{REMOTE_API_URL}/{uuid}/{msg}` for each chat line. |
| [patches/0002-wire-chat-forward-into-handler.patch](../patches/0002-wire-chat-forward-into-handler.patch) | Wires `wynn::on_incoming_chat` into the `ChatMessagePacket` handler and replies in-game with `[PicoLimbo] Code sent.` so the player has visible feedback. |

The path suffix `/api/auth` reflects what dazebot does with the data, not what PicoLimbo does. Don't rename it without coordinating with `../dazebot/api/main.py` (the receiver).

The "code sent" reply is intentionally only a transmission ack — dazebot's accept/reject outcome is asynchronous and not surfaced back through PicoLimbo. The legacy two-way HTTP server (server-initiated `/send` to specific players) lives on the `two-way-api-archive` branch; restore from there if server-push chat is ever needed.

## Build / deploy

- **Local:** `docker build -t picolimbo:local -f docker/Dockerfile .`.
- **Pin a different upstream ref locally:** `docker build --build-arg PICOLIMBO_REF=v1.13.1+mc26.2 ...` or edit the fallback default in `docker/Dockerfile`.
- **Production:** No host-side build. `upstream-track` CI resolves the latest upstream release tag from the GitHub API, builds, and publishes two tags on every master push / daily schedule:
  - `ghcr.io/wynncraft-veterans/auth-stack:<sanitized-ref>` — immutable, audit trail (e.g. `v1.13.1-mc26.2`).
  - `ghcr.io/wynncraft-veterans/auth-stack:rolling` — mutable, what vets-deploy's `IMAGE_TAG` points at by default. Watchtower picks up the new digest within ~24h.
- **Tag sanitization:** OCI tags can't contain `+`, so upstream `v1.12.2+mc26.1.2` is published as `v1.12.2-mc26.1.2`.

## Tracking upstream

Automatic. The `upstream-track` workflow runs daily at 06:00 UTC (and on every master push), queries `https://api.github.com/repos/Quozul/PicoLimbo/releases/latest`, and rebuilds against that tag. On a green build, both `:<sanitized-ref>` and `:rolling` are pushed. On a red build, **nothing** is pushed — `:rolling` keeps its previous digest, which is the implicit last-known-good state. GHCR is the state store; there is no separate tracking file.

When a build goes red:
- Inspect the failed `upstream-track` run. If `git apply` failed or compile failed, regenerate the affected patch — see [upstream-sync.md](upstream-sync.md).
- Production keeps running on the previous `:rolling` digest in the meantime; there's no urgent rollback needed.

When you need to manually pin (e.g. `:rolling` shipped a runtime-bad image before it was caught):
```
manage pin auth-stack v1.12.2-mc26.1.2     # pin to a specific known-good sanitized tag
manage pin auth-stack rolling              # resume auto-tracking
```
`manage pin` lives in vets-deploy's `scripts/manage.sh`. It writes `IMAGE_TAG` into the stack's `.env` and runs `docker compose pull && up -d`.

The `ARG PICOLIMBO_REF` default in [docker/Dockerfile](../docker/Dockerfile) is only used as the last-resort fallback when the GitHub API call fails AND for plain `docker build` invocations without `--build-arg`. CI overrides it on every run, so don't expect bumping it to change what production runs.

## Configuration

- `REMOTE_API_URL` env var — read by [pico_limbo/src/wynn.rs](../patches/0001-add-wynn-chat-forward-module.patch) at startup. In production, `http://dazebot:${DAZEBOT_PORT}/api/auth` (resolvable on the `verify` Docker network).
- [server.toml](../server.toml) — starter template. The *live* server.toml lives in `vets-deploy/stacks/picolimbo/server.toml` and is mounted RO into the container.

## Don't

- Don't add upstream PicoLimbo code (`crates/`, `pico_libraries/`, `pico_limbo/`, etc.) into this repo. The whole structural point is that they live only in upstream.
- Don't switch to `ghcr.io/quozul/picolimbo` — it has no chat forwarding and link codes will silently never resolve.
- Don't enable any feature that filters or transforms chat before the forward fires — dazebot needs the raw line.
- Don't add authentication to the chat forward — gated by Docker network isolation; adding shared-secret headers would force a coordinated change in dazebot too.

## What this repo is **not**

- Not a fork (in the git-history sense). It used to be a fork of `PierreV23/wynnvetserver`; that ancestry is now archived. Master is a clean overlay-shape branch.
- Not a Wynncraft-aware server. PicoLimbo is a literal limbo — players just stand around. The only logic added is the chat-line forward.
- Not part of the vetsmod auth path. That uses dazebot's separate `POST /api/auth/introspect` endpoint and never touches PicoLimbo.
