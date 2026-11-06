//! Polls.

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::emoji::PartialEmoji;
use crate::user::User;

/// A poll in a message.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Poll {
    /// The question. Only `text` is supported.
    pub question: PollMedia,
    /// The answers.
    pub answers: Vec<PollAnswer>,
    /// When the poll ends. `None` is reserved for polls without an end.
    #[serde(default)]
    pub expiry: Option<Timestamp>,
    /// Tells if users can select more than one answer.
    #[serde(default)]
    pub allow_multiselect: bool,
    /// The layout type.
    #[serde(default = "default_layout")]
    pub layout_type: PollLayoutType,
    /// The results. Absent means that the results are not known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results: Option<PollResults>,
}

fn default_layout() -> PollLayoutType {
    PollLayoutType::Default
}

/// A request to make a poll.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PollCreate {
    /// The question. Only `text` is supported.
    pub question: PollMedia,
    /// The answers, up to 10.
    pub answers: Vec<PollAnswer>,
    /// How many hours the poll is open, up to 32 days. The default is 24.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// Tells if users can select more than one answer.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub allow_multiselect: bool,
    /// The layout type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_type: Option<PollLayoutType>,
}

impl PollCreate {
    /// Makes a poll with a question and answers.
    #[must_use]
    pub fn new(question: impl Into<String>, answers: impl IntoIterator<Item = PollMedia>) -> Self {
        Self {
            question: PollMedia::text(question),
            answers: answers.into_iter().map(PollAnswer::new).collect(),
            duration: None,
            allow_multiselect: false,
            layout_type: None,
        }
    }

    /// Sets how many hours the poll is open.
    #[must_use]
    pub fn duration_hours(mut self, hours: u32) -> Self {
        self.duration = Some(hours);
        self
    }

    /// Sets whether users can select more than one answer.
    #[must_use]
    pub fn allow_multiselect(mut self, allow: bool) -> Self {
        self.allow_multiselect = allow;
        self
    }
}

crate::discord_enum! {
    /// The layout of a poll.
    pub enum PollLayoutType: u8 {
        /// The default layout.
        Default = 1,
    }
}

/// The text and emoji of a poll question or answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PollMedia {
    /// The text: up to 300 characters for a question, 55 for an answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<PartialEmoji>,
}

impl PollMedia {
    /// Makes media with text.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            emoji: None,
        }
    }

    /// Sets the emoji.
    #[must_use]
    pub fn emoji(mut self, emoji: PartialEmoji) -> Self {
        self.emoji = Some(emoji);
        self
    }
}

impl From<&str> for PollMedia {
    fn from(text: &str) -> Self {
        Self::text(text)
    }
}

/// An answer of a poll.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PollAnswer {
    /// The answer ID. Only in responses. The IDs are 1, 2, 3, and so on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_id: Option<u32>,
    /// The text and emoji.
    pub poll_media: PollMedia,
}

impl PollAnswer {
    /// Makes an answer for a request.
    #[must_use]
    pub fn new(media: PollMedia) -> Self {
        Self {
            answer_id: None,
            poll_media: media,
        }
    }
}

/// The results of a poll.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PollResults {
    /// Tells if the votes are counted exactly (the poll ended).
    #[serde(default)]
    pub is_finalized: bool,
    /// The count for each answer.
    #[serde(default)]
    pub answer_counts: Vec<PollAnswerCount>,
}

/// The number of votes for an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PollAnswerCount {
    /// The answer ID.
    pub id: u32,
    /// The number of votes.
    pub count: u64,
    /// Tells if the current user voted for the answer.
    #[serde(default)]
    pub me_voted: bool,
}

/// The users that voted for an answer, as `GET /channels/{id}/polls/{id}/answers/{id}` gives them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PollAnswerVoters {
    /// The users.
    pub users: Vec<User>,
}
