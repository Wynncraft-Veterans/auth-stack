# auth-stack

Patch overlay over [Quozul/PicoLimbo](https://github.com/Quozul/PicoLimbo) that
adds chat-line forwarding to dazebot for Minecraft account-link verification.
Production server: `verify.wynnvets.org:25565`.

## What this repo holds

```
patches/                          The entire functional delta (2 patches, ~50 LOC).
docker/Dockerfile                 Clones upstream + applies patches + builds.
docker-compose.yml                Local dev convenience (port 30066).
.github/workflows/                CI: verify-patches (per-PR) + upstream-smoke (nightly).
server.toml                       Starter config template (the live one lives in vets-deploy).
.claude/                          Project docs (CLAUDE.md, upstream-sync.md).
```

No upstream PicoLimbo source lives in this repo. The Docker build clones it
fresh from `Quozul/PicoLimbo` at a configurable ref.

## How it works

1. The Dockerfile's `source` stage clones `Quozul/PicoLimbo.git` at
   `PICOLIMBO_REF` (default = the pinned tag in `docker/Dockerfile`).
2. `patches/*.patch` are applied with `git apply --check` first, then
   `git apply`. Application failure aborts the build with the offending
   patch named in the log.
3. The `builder` stage compiles `pico_limbo` against the patched tree.
4. The final stage is upstream's distroless runtime layout.

The two patches are:

- `0001-add-wynn-chat-forward-module.patch` — adds `pico_limbo/src/wynn.rs`
  with `REMOTE_API_URL` env var + the helper that GETs
  `{REMOTE_API_URL}/{uuid}/{msg}` per chat line.
- `0002-wire-chat-forward-into-handler.patch` — wires `wynn::on_incoming_chat`
  into `ChatMessagePacket` and acks each line with `[PicoLimbo] Code sent.`.

## Build / run locally

```bash
docker build -t picolimbo:local -f docker/Dockerfile .
docker run --rm -p 25565:25565 -e REMOTE_API_URL=http://host.docker.internal:9421/api/auth picolimbo:local
```

Or via compose:
```bash
docker compose up --build
```

## Tracking upstream

There is no manual sync. The Dockerfile clones at `PICOLIMBO_REF`; bumping
that var (in `docker/Dockerfile` or via build-arg) is the entire upgrade
process. CI's `upstream-smoke` workflow rebuilds nightly against
`PICOLIMBO_REF=master` and fails if patches no longer apply or the build
no longer compiles — that is the early-warning signal.

When `upstream-smoke` fails:

1. Read the job log to identify which patch broke (`git apply --check` names
   the file) or which compile error appeared.
2. Locally regenerate the patch:
   ```bash
   git clone https://github.com/Quozul/PicoLimbo.git /tmp/pl && cd /tmp/pl
   git apply /path/to/auth-stack/patches/*.patch || # fix conflicts
   # edit files
   git diff > /path/to/auth-stack/patches/000X-foo.patch
   ```
3. Open a PR; `verify-patches` will gate it.

For a deeper guide see [.claude/upstream-sync.md](.claude/upstream-sync.md).

## Don't

- Don't switch production to `ghcr.io/quozul/picolimbo` — it has no chat
  forwarding and link codes will silently never resolve.
- Don't add upstream code (`crates/`, `pico_libraries/`, `pico_limbo/`)
  back into this repo. The whole point is that they live only in upstream.
- Don't add authentication to the chat forward — the `verify` Docker
  network already isolates it.

## License

MIT, inherited from upstream PicoLimbo. The patches are likewise released
under MIT — see [LICENSE](LICENSE).
