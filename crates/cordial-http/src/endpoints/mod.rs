//! One request type for each endpoint of the Discord API.
//!
//! Each request type has setters and implements `IntoFuture`. Write `.await` to send it.

pub mod application;
pub mod audit_log;
pub mod auto_moderation;
pub mod ban;
pub mod channel;
pub mod command;
pub mod emoji;
pub mod gateway;
pub mod guild;
pub mod guild_settings;
pub mod identity;
pub mod interaction;
pub mod invite;
pub mod join_request;
mod limits;
pub mod lobby;
pub mod member;
pub mod message;
pub mod monetization;
pub mod oauth2;
pub mod partner_sdk;
pub mod poll;
pub mod reaction;
pub mod role;
pub mod scheduled_event;
pub mod soundboard;
pub mod stage;
pub mod sticker;
pub mod template;
pub mod thread;
pub mod user;
pub mod voice;
pub mod webhook;

/// Implements `IntoFuture` for a request type with a `client` field.
macro_rules! into_future {
    ($($name:ident),* $(,)?) => {
        $(
            impl ::std::future::IntoFuture for $name {
                type Output = ::std::result::Result<<Self as crate::request::Endpoint>::Response, crate::error::Error>;
                type IntoFuture = crate::transport::BoxFuture<'static, Self::Output>;

                fn into_future(self) -> Self::IntoFuture {
                    Box::pin(async move { self.client.request(&self).await })
                }
            }
        )*
    };
}

pub(crate) use into_future;
