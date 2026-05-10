# auth-stack

A fork of [Quozul/PicoLimbo](https://github.com/Quozul/PicoLimbo) (Rust ultra-light Minecraft limbo server) with a single, narrow modification: every chat line a player types is forwarded to a remote HTTP endpoint, used by dazebot to consume Minecraft-account link codes.

## Key facts

- **Production address:** `verify.wynnvets.org:25565` (Minecraft TCP)
- **Stack:** Rust workspace (multi-crate). The actual binary is `pico_limbo`.
- **Ancestry:** upstream `Quozul/PicoLimbo` → fork `PierreV23/wynnvetserver` → this fork `Wynncraft-Veterans/auth-stack` (the rename happened to make the verification purpose obvious).
- **Why this fork exists:** Upstream has no chat-forwarding mechanism; that's what dazebot's link flow needs.

## Related repos (same workspace)

- `../dazebot` — Receives the chat-line forwards at `GET /api/auth/{uuid}/{msg}` and looks for `LinkCode` matches. **This is the only consumer of auth-stack's HTTP forwarding.**
- `../vets-deploy` — Compose stack at `stacks/picolimbo/` runs this image (built locally from a sibling clone).
- `../vetsmod` — Indirect relationship: vetsmod users are Discord-linked via the auth-stack→dazebot chain *before* they ever run `/vetsmod` to get a vetsmod auth key. Two distinct auth flows that happen to share dazebot's `/api/auth/` HTTP prefix.
- `../temporary-server` — Unrelated.

## Where the modification lives

| File | What it does |
|------|--------------|
| [pico_limbo/src/wynn.rs](../pico_limbo/src/wynn.rs) | `REMOTE_API_URL` env var (defaults to `http://localhost:9421/api/auth`) and the helper that GETs `{REMOTE_API_URL}/{uuid}/{msg}` for each chat line. |
| [pico_limbo/src/handlers/play/commands.rs](../pico_limbo/src/handlers/play/commands.rs) | The `ChatMessagePacket` handler that fires `wynn::on_incoming_chat()` and replies in-game with `[PicoLimbo] Code sent.` so the player has visible feedback that their message was forwarded. |

The path suffix `/api/auth` reflects what dazebot does with the data, not what PicoLimbo does. Don't rename it without coordinating with `../dazebot/api/main.py` (the receiver).

The "code sent" reply is intentionally only a transmission ack — dazebot's accept/reject outcome is asynchronous and not surfaced back through PicoLimbo (the legacy two-way registry was archived in the `two-way-api-archive` branch; restore from there if you ever need server-push chat).

## Build / deploy

- **Local:** `cargo build --release -p pico_limbo` (toolchain pinned in `rust-toolchain.toml`).
- **Container:** `docker/Dockerfile` is the build context the vets-deploy stack uses. The Docker build context **must** include `pico_libraries/` (recently fixed — see commit `ffc3e46`); upstream registry data lives there.
- **Production:**
  ```bash
  cd /opt/docker/picolimbo/src
  git pull
  manage update picolimbo
  ```

## Tracking upstream

This fork keeps in sync with `Quozul/PicoLimbo`. The most recent merge brought the workspace to **v1.12.2+mc26.1.2** (support for MC 26.1.x clients via the new `pico_nbt` / `pico_registries` / `identifier` crates, dialog registry, transfer packets, etc.). Don't edit the upstream-owned crates (`crates/*`, `pico_libraries/*`) unless you intend to maintain a permanent divergence — every conflict you create is a tax on the next merge.

Concrete merge guide with conflict rules: [upstream-sync.md](upstream-sync.md).

## Configuration

[server.toml](../server.toml) — bind address, MOTD, default game-mode, etc. The fork keeps upstream PicoLimbo's config schema intact; the only added knob is the `REMOTE_API_URL` env var read at runtime.

In production, `REMOTE_API_URL` is typically set to `http://dazebot:${DAZEBOT_PORT}/api/auth` (resolvable on the `verify` Docker network).

## Don't

- Don't switch to the upstream `ghcr.io/quozul/picolimbo` image — it has no chat forwarding and link codes will silently never resolve.
- Don't enable any feature that filters or transforms chat before the forward fires — dazebot needs the raw line.
- Don't add authentication to the chat forward — it's already gated by Docker network isolation; adding shared-secret headers would force a coordinated change in dazebot too.

## What this repo is **not**

- Not a Wynncraft-aware server. PicoLimbo is a literal limbo — players just stand around. The only logic added is the chat-line forward.
- Not part of the vetsmod auth path. That uses dazebot's separate `POST /api/auth/introspect` endpoint and never touches PicoLimbo.
