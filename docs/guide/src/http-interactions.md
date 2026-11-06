# HTTP interactions

A bot can get interactions over HTTP in place of the gateway. Discord sends a POST request
to a URL of the bot for each interaction. This is good for a bot with only commands and
components:

- The bot needs no gateway connection and no shards.
- The bot can run on a serverless platform or behind a load balancer.

The bot gets no gateway events. Event handlers, the cache, and collectors do not work
without the gateway.

## The endpoint crate

The crate `cordial-endpoint` (module `cordial::endpoint`) receives the requests:

1. It checks the Ed25519 signature of each request with the public key of the application.
2. It rejects a request with a timestamp older than 5 minutes. `Endpoint::max_age` changes
   this.
3. It answers the PING of Discord.
4. It gives each interaction to a `Handler`.

The core is independent of the HTTP server. Give the two signature headers and the raw
body to `Endpoint::handle`, and send back the `HttpResponse`.

| Feature of `cordial` | Gives |
|---|---|
| `endpoint` | The `Endpoint` type, and the HTTP source of the framework |
| `axum` | `cordial::endpoint::axum::router`, a ready `axum` router for `POST /` |

The endpoint also receives webhook events (the "Event Webhooks URL"). Implement
`Handler::webhook_event` for them.

## Use the framework over HTTP

The framework routes HTTP interactions to the same command, component, and modal handlers:

1. Build the bot as usual.
2. Call `Bot::prepare`. It gets the application ID and syncs the commands. Do not call
   `run`: it connects to the gateway.
3. Give `Bot::endpoint_handler` to `Endpoint::new`.
4. Serve the endpoint.

This is `crates/cordial/examples/http_interactions.rs`:

```rust
{{#include ../../../crates/cordial/examples/http_interactions.rs}}
```

Run it:

```sh
DISCORD_TOKEN=... DISCORD_PUBLIC_KEY=... cargo run --example http_interactions --features axum
```

## Set the URL in the Developer Portal

1. Make the server public with HTTPS, for example with a tunnel.
2. Copy the "Public Key" from the "General Information" page of the application.
3. Set the "Interactions Endpoint URL" on the same page, for example `https://example.com/`.
4. Click "Save Changes". Discord sends a PING to check the URL. The endpoint answers it.

Discord does not save the URL when the check fails. Make sure that the server runs and that
the public key is correct.

## Responses over HTTP

The handlers answer with the interaction callback endpoint, as they do for gateway
interactions. The endpoint handler waits up to 3 seconds for the initial response. Then
the HTTP request ends with status 202. All response methods, defers, and follow-up messages
work as in [Responses](responses.md).

A `Handler` without the framework can answer in the HTTP response itself with
`Reply::Response`. Such a response cannot carry files.
