//! Tests against the real Discord gateway. They need the network, so they are ignored by
//! default. Run them with `cargo test -p cordial-gateway --test live -- --ignored`.

use std::sync::Arc;
use std::time::Duration;

use cordial_gateway::{IdentifyQueue, Lifecycle, Shard, ShardConfig, ShardEvent};
use cordial_http::Token;
use cordial_http::transport::BoxFuture;
use cordial_model::gateway::Intents;

/// A queue that never gives a permit, so that the test never identifies.
#[derive(Debug)]
struct NeverQueue;

impl IdentifyQueue for NeverQueue {
    fn acquire(&self, _shard_id: u32) -> BoxFuture<'static, ()> {
        Box::pin(std::future::pending())
    }
}

#[tokio::test]
#[ignore = "needs the network"]
async fn receives_hello_from_discord() {
    let config = ShardConfig::new(Token::bot("not-a-token"), Intents::GUILDS)
        .identify_queue(Arc::new(NeverQueue));
    let (shard, mut events) = Shard::start(config);

    let hello = tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(message) = events.recv().await {
            if let ShardEvent::Lifecycle(Lifecycle::Hello { heartbeat_interval }) = message.event {
                return Some(heartbeat_interval);
            }
        }
        None
    })
    .await
    .expect("no Hello within 20 seconds")
    .expect("the shard stopped");

    assert!(hello > Duration::from_secs(10));
    shard.close(cordial_gateway::CloseMode::End).await;
}
