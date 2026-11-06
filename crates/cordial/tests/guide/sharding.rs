use cordial::framework::{Bot, Error};
use cordial::gateway::{ManagerError, ShardEvent, ShardManager, ShardRange};
use cordial::http::{Client, Token};
use cordial::model::gateway::{Event, Intents};

// ANCHOR: manager
async fn run_shards(token: Token) -> Result<(), ManagerError> {
    let http = Client::new(token);
    let mut manager = ShardManager::builder(http, Intents::GUILDS | Intents::GUILD_MESSAGES)
        .shards(ShardRange::Recommended)
        .start()
        .await?;

    while let Some(message) = manager.next_event().await {
        match message.event {
            ShardEvent::Dispatch(event) => {
                if let Event::Ready(ready) = *event {
                    tracing::info!(shard = message.shard_id, user = %ready.user.username, "ready");
                }
            }
            ShardEvent::Fatal(error) => {
                tracing::error!(shard = message.shard_id, %error, "the shard stopped");
                break;
            }
            _ => {}
        }
    }

    manager.close().await;
    Ok(())
}
// ANCHOR_END: manager

// ANCHOR: process
/// Process 1 of 2: shards 0 to 7 of 16. The other process runs shards 8 to 15.
fn first_process(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .shards(ShardRange::Some {
            ids: (0..8).collect(),
            count: 16,
        })
        .build()
}
// ANCHOR_END: process
