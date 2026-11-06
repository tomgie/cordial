//! Poll endpoints.

use cordial_model::id::{ChannelId, MessageId, UserId};
use cordial_model::message::Message;
use cordial_model::poll::PollAnswerVoters;

use crate::client::Client;
use crate::request::Endpoint;
use crate::route::{MajorParameter, Method, Query, Route};

/// `GET /channels/{channel.id}/polls/{message.id}/answers/{answer_id}`: the users that voted
/// for an answer.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetAnswerVoters {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    answer_id: u32,
    after: Option<UserId>,
    limit: Option<u8>,
}

impl GetAnswerVoters {
    /// Gets users after this user ID.
    pub fn after(mut self, user_id: UserId) -> Self {
        self.after = Some(user_id);
        self
    }

    /// Sets the number of users, 1 to 100. The default is 25.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }
}

impl Endpoint for GetAnswerVoters {
    type Response = PollAnswerVoters;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("after", self.after);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/polls/{message_id}/answers/{answer_id}",
            format!(
                "/channels/{}/polls/{}/answers/{}{}",
                self.channel_id,
                self.message_id,
                self.answer_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `POST /channels/{channel.id}/polls/{message.id}/expire`: ends a poll now.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EndPoll {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
}

impl Endpoint for EndPoll {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/polls/{message_id}/expire",
            format!(
                "/channels/{}/polls/{}/expire",
                self.channel_id, self.message_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn retry_safe(&self) -> bool {
        // An ended poll stays ended.
        true
    }
}

super::into_future!(GetAnswerVoters, EndPoll);

impl Client {
    /// Gets the users that voted for an answer of a poll.
    pub fn answer_voters(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        answer_id: u32,
    ) -> GetAnswerVoters {
        GetAnswerVoters {
            client: self.clone(),
            channel_id,
            message_id,
            answer_id,
            after: None,
            limit: None,
        }
    }

    /// Ends a poll now. The poll must be from the current user.
    pub fn end_poll(&self, channel_id: ChannelId, message_id: MessageId) -> EndPoll {
        EndPoll {
            client: self.clone(),
            channel_id,
            message_id,
        }
    }
}
