//! Voice endpoints: voice regions and voice states.

use cordial_model::id::{ChannelId, GuildId, UserId};
use cordial_model::voice::{VoiceRegion, VoiceState};
use cordial_model::{Patch, Timestamp};
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody};
use crate::route::{MajorParameter, Method, Route};

/// `GET /voice/regions`: the voice regions for the `rtc_region` of a channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetVoiceRegions {
    client: Client,
}

impl Endpoint for GetVoiceRegions {
    type Response = Vec<VoiceRegion>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/voice/regions",
            "/voice/regions".to_owned(),
            MajorParameter::None,
        )
    }
}

/// `GET /guilds/{guild.id}/voice-states/@me`: the voice state of the current user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentVoiceState {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetCurrentVoiceState {
    type Response = VoiceState;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/voice-states/@me",
            format!("/guilds/{}/voice-states/@me", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/voice-states/{user.id}`: the voice state of a user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetVoiceState {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
}

impl Endpoint for GetVoiceState {
    type Response = VoiceState;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/voice-states/{user_id}",
            format!("/guilds/{}/voice-states/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PATCH /guilds/{guild.id}/voice-states/@me`: edits the voice state of the current user
/// in a stage channel.
///
/// The current user must be in the stage channel already. To unsuppress itself, the user
/// needs `MUTE_MEMBERS`. To ask to speak, the user needs `REQUEST_TO_SPEAK`.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditCurrentVoiceState {
    client: Client,
    guild_id: GuildId,
    body: EditCurrentVoiceStateBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditCurrentVoiceStateBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suppress: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    request_to_speak_timestamp: Patch<Timestamp>,
}

impl EditCurrentVoiceState {
    /// Sets the stage channel that the user is in.
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.channel_id = Some(channel_id);
        self
    }

    /// Sets whether the user is suppressed (cannot speak).
    pub fn suppress(mut self, suppress: bool) -> Self {
        self.body.suppress = Some(suppress);
        self
    }

    /// Asks to speak at a time. The time can be now or in the future.
    pub fn request_to_speak_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.body.request_to_speak_timestamp = Patch::Set(timestamp);
        self
    }

    /// Removes the request to speak.
    pub fn clear_request_to_speak(mut self) -> Self {
        self.body.request_to_speak_timestamp = Patch::Clear;
        self
    }
}

impl Endpoint for EditCurrentVoiceState {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/voice-states/@me",
            format!("/guilds/{}/voice-states/@me", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `PATCH /guilds/{guild.id}/voice-states/{user.id}`: edits the voice state of a user in a
/// stage channel.
///
/// The user must be in the stage channel already. The current user needs `MUTE_MEMBERS`.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditVoiceState {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    body: EditVoiceStateBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditVoiceStateBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suppress: Option<bool>,
}

impl EditVoiceState {
    /// Sets the stage channel that the user is in.
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.channel_id = Some(channel_id);
        self
    }

    /// Sets whether the user is suppressed (cannot speak).
    pub fn suppress(mut self, suppress: bool) -> Self {
        self.body.suppress = Some(suppress);
        self
    }
}

impl Endpoint for EditVoiceState {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/voice-states/{user_id}",
            format!("/guilds/{}/voice-states/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

super::into_future!(
    GetVoiceRegions,
    GetCurrentVoiceState,
    GetVoiceState,
    EditCurrentVoiceState,
    EditVoiceState
);

impl Client {
    /// Gets the voice regions.
    pub fn voice_regions(&self) -> GetVoiceRegions {
        GetVoiceRegions {
            client: self.clone(),
        }
    }

    /// Gets the voice state of the current user in a guild.
    pub fn current_voice_state(&self, guild_id: GuildId) -> GetCurrentVoiceState {
        GetCurrentVoiceState {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets the voice state of a user in a guild.
    pub fn voice_state(&self, guild_id: GuildId, user_id: UserId) -> GetVoiceState {
        GetVoiceState {
            client: self.clone(),
            guild_id,
            user_id,
        }
    }

    /// Edits the voice state of the current user in a stage channel.
    pub fn edit_current_voice_state(&self, guild_id: GuildId) -> EditCurrentVoiceState {
        EditCurrentVoiceState {
            client: self.clone(),
            guild_id,
            body: EditCurrentVoiceStateBody::default(),
        }
    }

    /// Edits the voice state of a user in a stage channel.
    pub fn edit_voice_state(&self, guild_id: GuildId, user_id: UserId) -> EditVoiceState {
        EditVoiceState {
            client: self.clone(),
            guild_id,
            user_id,
            body: EditVoiceStateBody::default(),
        }
    }
}
