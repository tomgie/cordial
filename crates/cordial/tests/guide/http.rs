use std::time::Duration;

use cordial::http::{Client, Error, FileUpload, JsonErrorCode, Token};
use cordial::model::embed::Embed;
use cordial::model::id::{ChannelId, MessageId};

// ANCHOR: client
fn make_client() -> Result<Client, std::env::VarError> {
    let client = Client::builder()
        .token(Token::from_env("DISCORD_TOKEN")?)
        .user_agent_suffix("my-bot/1.0")
        .timeout(Duration::from_secs(15))
        .build();
    Ok(client)
}
// ANCHOR_END: client

// ANCHOR: send
async fn send_report(client: &Client, channel_id: ChannelId) -> Result<MessageId, Error> {
    let file =
        FileUpload::new("report.txt", b"All systems work.".as_slice()).content_type("text/plain");
    let embed = Embed::new()
        .title("Daily report")
        .description("The report is in the file.");

    let message = client
        .create_message(channel_id)
        .content("Here is the daily report.")
        .embed(embed)
        .file(file)
        .await?;
    Ok(message.id)
}
// ANCHOR_END: send

// ANCHOR: errors
/// Deletes a message. A message that is already gone is not an error.
async fn delete_quietly(
    client: &Client,
    channel_id: ChannelId,
    message_id: MessageId,
) -> Result<(), Error> {
    match client.delete_message(channel_id, message_id).await {
        Ok(()) => Ok(()),
        Err(error) if error.code() == Some(JsonErrorCode::UnknownMessage) => Ok(()),
        Err(error) => {
            if let Some(api) = error.api() {
                for field in api.field_errors() {
                    tracing::warn!(path = field.path, code = field.code, "{}", field.message);
                }
            }
            Err(error)
        }
    }
}
// ANCHOR_END: errors

// ANCHOR: pages
/// Counts the messages of a channel that have a word.
async fn count_word(client: &Client, channel_id: ChannelId, word: &str) -> Result<usize, Error> {
    let mut count = 0;
    let mut history = client.message_history(channel_id);
    while let Some(page) = history.next_page().await? {
        count += page
            .iter()
            .filter(|message| message.content.contains(word))
            .count();
    }
    Ok(count)
}
// ANCHOR_END: pages
