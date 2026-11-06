//! User endpoints.

use cordial_model::id::UserId;
use cordial_model::user::User;

use crate::client::Client;
use crate::request::Endpoint;
use crate::route::{MajorParameter, Method, Route};

/// `GET /users/@me`: the current user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentUser {
    client: Client,
}

impl Endpoint for GetCurrentUser {
    type Response = User;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/@me",
            "/users/@me".to_owned(),
            MajorParameter::None,
        )
    }
}

/// `GET /users/{user.id}`: a user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetUser {
    client: Client,
    user_id: UserId,
}

impl Endpoint for GetUser {
    type Response = User;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/{user_id}",
            format!("/users/{}", self.user_id),
            MajorParameter::None,
        )
    }
}

super::into_future!(GetCurrentUser, GetUser);

impl Client {
    /// Gets the current user.
    pub fn current_user(&self) -> GetCurrentUser {
        GetCurrentUser {
            client: self.clone(),
        }
    }

    /// Gets a user.
    pub fn user(&self, user_id: UserId) -> GetUser {
        GetUser {
            client: self.clone(),
            user_id,
        }
    }
}
