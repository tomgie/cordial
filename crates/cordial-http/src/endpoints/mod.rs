//! One request type for each endpoint of the Discord API.
//!
//! Each request type has setters and implements `IntoFuture`. Write `.await` to send it.

pub mod application;
pub mod channel;
pub mod command;
pub mod emoji;
pub mod gateway;
pub mod invite;
pub mod message;
pub mod poll;
pub mod stage;
pub mod sticker;
pub mod thread;
pub mod user;
pub mod voice;

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
