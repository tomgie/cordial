//! Reaction endpoints.

use cordial_model::emoji::ReactionType;
use cordial_model::id::{ChannelId, MessageId, UserId};
use cordial_model::message::ReactionKind;
use cordial_model::user::User;

use crate::client::Client;
use crate::request::Endpoint;
use crate::route::{MajorParameter, Method, Query, Route, encode_segment};

/// Gives the emoji as an encoded path segment: `name:id` for a custom emoji.
fn emoji_segment(emoji: &ReactionType) -> String {
    encode_segment(&emoji.to_string())
}

/// `PUT /channels/{channel.id}/messages/{message.id}/reactions/{emoji}/@me`: adds a reaction
/// of the current user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateReaction {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    emoji: ReactionType,
}

impl Endpoint for CreateReaction {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me",
            format!(
                "/channels/{}/messages/{}/reactions/{}/@me",
                self.channel_id,
                self.message_id,
                emoji_segment(&self.emoji)
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji}/@me`: removes a
/// reaction of the current user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteOwnReaction {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    emoji: ReactionType,
}

impl Endpoint for DeleteOwnReaction {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me",
            format!(
                "/channels/{}/messages/{}/reactions/{}/@me",
                self.channel_id,
                self.message_id,
                emoji_segment(&self.emoji)
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji}/{user.id}`: removes
/// the reaction of a different user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteUserReaction {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    emoji: ReactionType,
    user_id: UserId,
}

impl Endpoint for DeleteUserReaction {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/{user_id}",
            format!(
                "/channels/{}/messages/{}/reactions/{}/{}",
                self.channel_id,
                self.message_id,
                emoji_segment(&self.emoji),
                self.user_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/messages/{message.id}/reactions/{emoji}`: the users that
/// reacted with an emoji.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetReactions {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    emoji: ReactionType,
    kind: Option<ReactionKind>,
    after: Option<UserId>,
    limit: Option<u8>,
}

impl GetReactions {
    /// Sets the type of reaction: normal or super. The default is normal.
    pub fn kind(mut self, kind: ReactionKind) -> Self {
        self.kind = Some(kind);
        self
    }

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

impl Endpoint for GetReactions {
    type Response = Vec<User>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("type", self.kind.map(u8::from));
        query.push_opt("after", self.after);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
            format!(
                "/channels/{}/messages/{}/reactions/{}{}",
                self.channel_id,
                self.message_id,
                emoji_segment(&self.emoji),
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `DELETE /channels/{channel.id}/messages/{message.id}/reactions`: removes all reactions of a
/// message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteAllReactions {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
}

impl Endpoint for DeleteAllReactions {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/messages/{message_id}/reactions",
            format!(
                "/channels/{}/messages/{}/reactions",
                self.channel_id, self.message_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `DELETE /channels/{channel.id}/messages/{message.id}/reactions/{emoji}`: removes all
/// reactions with one emoji.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteAllReactionsForEmoji {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    emoji: ReactionType,
}

impl Endpoint for DeleteAllReactionsForEmoji {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
            format!(
                "/channels/{}/messages/{}/reactions/{}",
                self.channel_id,
                self.message_id,
                emoji_segment(&self.emoji)
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

super::into_future!(
    CreateReaction,
    DeleteOwnReaction,
    DeleteUserReaction,
    GetReactions,
    DeleteAllReactions,
    DeleteAllReactionsForEmoji
);

impl Client {
    /// Adds a reaction of the current user to a message.
    pub fn create_reaction(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        emoji: impl Into<ReactionType>,
    ) -> CreateReaction {
        CreateReaction {
            client: self.clone(),
            channel_id,
            message_id,
            emoji: emoji.into(),
        }
    }

    /// Removes a reaction of the current user from a message.
    pub fn delete_own_reaction(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        emoji: impl Into<ReactionType>,
    ) -> DeleteOwnReaction {
        DeleteOwnReaction {
            client: self.clone(),
            channel_id,
            message_id,
            emoji: emoji.into(),
        }
    }

    /// Removes the reaction of a user from a message.
    pub fn delete_user_reaction(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        emoji: impl Into<ReactionType>,
        user_id: UserId,
    ) -> DeleteUserReaction {
        DeleteUserReaction {
            client: self.clone(),
            channel_id,
            message_id,
            emoji: emoji.into(),
            user_id,
        }
    }

    /// Gets the users that reacted to a message with an emoji.
    pub fn reactions(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        emoji: impl Into<ReactionType>,
    ) -> GetReactions {
        GetReactions {
            client: self.clone(),
            channel_id,
            message_id,
            emoji: emoji.into(),
            kind: None,
            after: None,
            limit: None,
        }
    }

    /// Removes all reactions from a message.
    pub fn delete_all_reactions(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> DeleteAllReactions {
        DeleteAllReactions {
            client: self.clone(),
            channel_id,
            message_id,
        }
    }

    /// Removes all reactions with one emoji from a message.
    pub fn delete_all_reactions_for_emoji(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        emoji: impl Into<ReactionType>,
    ) -> DeleteAllReactionsForEmoji {
        DeleteAllReactionsForEmoji {
            client: self.clone(),
            channel_id,
            message_id,
            emoji: emoji.into(),
        }
    }
}
