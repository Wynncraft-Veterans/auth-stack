# auth-stack

Patch overlay over [Quozul/PicoLimbo](https://github.com/Quozul/PicoLimbo) that
adds chat-line forwarding to dazebot for Minecraft account-link verification.
Production server: `verify.wynnvets.org:25565`.

## What this repo holds

```
patches/                          The entire functional delta (7 patches).
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

The four patches are:

- `0001-add-wynn-chat-forward-module.patch` — adds `pico_limbo/src/wynn.rs`
  and registers the module. Owns `WYNN_CHAT_ROUTES` (a comma-separated
  `prefix=url` list, blank prefix = default backend), the longest-prefix
  match, the forward itself, and the `ChatOutcome` it parses back out.
  The request is bounded at 5s and goes through one shared client.
  `REMOTE_API_URL` still works as a default-only route, so a
  single-backend deployment needs no env change.
- `0002-wire-chat-forward-into-the-chat-handler.patch` — calls
  `wynn::on_incoming_chat` from `ChatMessagePacket` and acts on the
  outcome: disconnect on `kick_message`, reply in chat on
  `chat_message`, otherwise ack the line with `[PicoLimbo] Code sent.`.
- `0003-render-kick-reasons-as-mini-message.patch` — disconnect packets
  take a pre-styled component, so a kick reason can carry colour.
- `0004-drain-the-socket-before-closing-a-kicked-client.patch` — read and
  discard whatever the client still had in flight before letting the
  socket drop. See [Why the kick needs a drain](#why-the-kick-needs-a-drain).

**Each patch owns a disjoint set of files, and each applies to a pristine
upstream tree on its own.** That's deliberate: an upstream bump only ever
conflicts with the patches touching the file upstream changed, and those
regenerate independently. The series was seven patches for a while, staged
in the order the work happened — which meant `wynn.rs` was rewritten by
four of them and `commands.rs` by three, so half the series described code
the other half deleted, and a conflict in an early patch cascaded through
every later one. Patch files are a diff, not a history; auth-stack's git
log is the history.

When you add one, group it by the file it touches rather than appending it
chronologically.

## Why the kick needs a drain

Upstream's `ClientData::shutdown` sends FIN and lets the `TcpStream`
drop. Closing a socket that still has unread data in its receive queue
makes the kernel send RST rather than finish the graceful close, and an
RST tells the peer to discard its receive buffer — including a Disconnect
packet written milliseconds earlier. The player sees
`SocketException: Connection reset`, or on Windows
`IOException: An established connection was aborted by the software in
your host machine`, and never sees the kick screen.

The chat forward is what turns this from theoretical into routine: it
runs inside the packet handler via `block_in_place`, so for the 300ms–1s
it takes, nothing is reading the socket. Everything the player's client
sends in that window — movement, keep-alive replies — is still queued
when the kick lands. Whether it bites depends on whether the player
happened to move, which is why the same code disconnects cleanly on one
attempt and resets on the next.

Patch `0004` reads and discards until the client closes its own half or
500ms passes, so the queue is empty by the time the socket drops. It
applies to every disconnect path, not just the chat-forward one.

## Chat-forward contract

Each routed backend answers with JSON:

```json
{"kick_message": "…or null", "chat_message": "…or null"}
```

`kick_message` disconnects the player with that text. Otherwise
`chat_message` is sent back in chat and the player stays connected.
Otherwise the line is acked with `Code sent.`. Both absent is the normal
case for dazebot, whose link probe ignores non-code lines.

Both are rendered as MiniMessage, so a backend can colour them:
`<gold>`, `<gray>`, `<green>`, `<aqua>`, `<bold>` and the rest of the
vanilla names, plus `<newline>`. Plain prose contains no tags and renders
as itself, so a backend that sends none is unaffected.

The forward is bounded at 5 seconds. It runs inside the packet handler
via `block_in_place`, so the player's connection isn't serviced while
it's outstanding — no keep-alives, no reads. Unbounded, a slow backend
stops looking like a slow backend and starts looking to the player like
`IOException: connection aborted`. On timeout they're told to try again
rather than left guessing. That same stall is what made the missing
socket drain bite; see above.

Reserve the kick for text the player needs *after* the session ends —
nothing on a Minecraft disconnect screen is clickable or scrollable, so
it suits a one-shot code and little else. Rejections belong in chat.

## Build / run locally

```bash
docker build -t picolimbo:local -f docker/Dockerfile .
# Single-backend (unchanged):
docker run --rm -p 25565:25565 -e REMOTE_API_URL=http://host.docker.internal:9421/api/auth picolimbo:local
# Multi-backend (prefix-routed; blank prefix = default):
docker run --rm -p 25565:25565 \
  -e WYNN_CHAT_ROUTES='=http://host.docker.internal:9421/api/auth,hall=http://host.docker.internal:9423/api/verify' \
  picolimbo:local
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
