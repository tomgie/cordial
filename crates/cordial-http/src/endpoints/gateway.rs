//! Gateway endpoints.

use cordial_model::gateway::{GatewayBotInfo, GatewayInfo};

use crate::client::Client;
use crate::request::Endpoint;
use crate::route::{Authentication, MajorParameter, Method, Route};

/// `GET /gateway`: the gateway URL. Needs no token.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGateway {
    client: Client,
}

impl Endpoint for GetGateway {
    type Response = GatewayInfo;

    fn route(&self) -> Route {
        let mut route = Route::new(
            Method::Get,
            "/gateway",
            "/gateway".to_owned(),
            MajorParameter::None,
        );
        route.authentication = Authentication::None;
        route
    }
}

/// `GET /gateway/bot`: the gateway URL, the recommended shard count, and the session start
/// limit.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGatewayBot {
    client: Client,
}

impl Endpoint for GetGatewayBot {
    type Response = GatewayBotInfo;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/gateway/bot",
            "/gateway/bot".to_owned(),
            MajorParameter::None,
        )
    }
}

super::into_future!(GetGateway, GetGatewayBot);

impl Client {
    /// Gets the gateway URL.
    pub fn gateway(&self) -> GetGateway {
        GetGateway {
            client: self.clone(),
        }
    }

    /// Gets the gateway URL, the recommended shard count, and the session start limit.
    pub fn gateway_bot(&self) -> GetGatewayBot {
        GetGatewayBot {
            client: self.clone(),
        }
    }
}
