use uuid::Uuid;

use std::env;
use std::sync::LazyLock;

// configuration
//
// REMOTE_API_URL is the base URL that every in-game chat line is GET'd against
// as `{REMOTE_API_URL}/{uuid}/{msg}`. The receiving service is *not* a chat
// bridge — it is dazebot's account-link probe, which only acts on messages
// containing an active link code and silently drops everything else. The
// path suffix `/api/auth` reflects that purpose. Override with the env var
// when running against a non-default deployment.
//
// The reverse direction (server-initiated chat to a specific player via an
// HTTP `/send` endpoint and an in-process registry) lived here historically;
// it has been moved to the `two-way-api-archive` branch and removed from
// master because dazebot does not consume it. Restore from that branch if
// you need server-push chat in the future.
static REMOTE_API_URL: LazyLock<String> = LazyLock::new(|| {
    env::var("REMOTE_API_URL").unwrap_or_else(|_| "http://localhost:9421/api/auth".to_string())
});

// fires when a player sends a message in the minecraft chat
pub fn on_incoming_chat(uuid: Uuid, msg: String) {
    tokio::spawn(async move {
        let base_url = &*REMOTE_API_URL;
        let url = format!("{base_url}/{uuid}/{msg}");
        let _ = reqwest::get(&url).await;
    });
}
