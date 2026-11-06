//! Channel endpoints.

use cordial_model::channel::Channel;
use cordial_model::id::ChannelId;

use crate::client::Client;
use crate::request::Endpoint;
use crate::route::{MajorParameter, Method, Route};

/// `GET /channels/{channel.id}`: a channel or a thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChannel {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for GetChannel {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/channels/{channel_id}",
            format!("/channels/{}", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

super::into_future!(GetChannel);

impl Client {
    /// Gets a channel or a thread.
    pub fn channel(&self, channel_id: ChannelId) -> GetChannel {
        GetChannel {
            client: self.clone(),
            channel_id,
        }
    }
}
