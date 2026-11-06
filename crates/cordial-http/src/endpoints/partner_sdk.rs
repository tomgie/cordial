//! Discord Social SDK partner endpoints: provisional accounts and DM moderation.
//!
//! The OpenAPI specification is the main source of these endpoints. The Social SDK guides
//! show how to use them.
//!
//! | Endpoint | Authentication |
//! |---|---|
//! | [`CreateProvisionalToken`] | None. The body has the client ID and the client secret. |
//! | [`UnmergeProvisionalAccount`] | None. The body has the client ID and the client secret. |
//! | [`CreateBotProvisionalToken`] | Bot token |
//! | [`UnmergeBotProvisionalAccount`] | Bot token |
//! | [`UpdateDmMessageModerationMetadata`] | Bot token |

use std::borrow::Cow;
use std::fmt;

use cordial_model::id::{ApplicationId, MessageId, UserId};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::lobby::{Metadata, check_moderation_metadata};
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Route};
use crate::token::Token;

/// The type of an external authentication token.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExternalAuthType(Cow<'static, str>);

impl ExternalAuthType {
    /// An OpenID Connect ID token.
    pub const OIDC: Self = Self::from_static("OIDC");
    /// An Epic Online Services access token.
    pub const EPIC_ONLINE_SERVICES_ACCESS_TOKEN: Self =
        Self::from_static("EPIC_ONLINE_SERVICES_ACCESS_TOKEN");
    /// An Epic Online Services ID token.
    pub const EPIC_ONLINE_SERVICES_ID_TOKEN: Self =
        Self::from_static("EPIC_ONLINE_SERVICES_ID_TOKEN");
    /// A Steam session ticket.
    pub const STEAM_SESSION_TICKET: Self = Self::from_static("STEAM_SESSION_TICKET");
    /// A Unity Services ID token.
    pub const UNITY_SERVICES_ID_TOKEN: Self = Self::from_static("UNITY_SERVICES_ID_TOKEN");
    /// An access token that Discord issued for the bot of the application.
    pub const DISCORD_BOT_ISSUED_ACCESS_TOKEN: Self =
        Self::from_static("DISCORD_BOT_ISSUED_ACCESS_TOKEN");
    /// An Apple ID token.
    pub const APPLE_ID_TOKEN: Self = Self::from_static("APPLE_ID_TOKEN");
    /// A PlayStation Network ID token.
    pub const PLAYSTATION_NETWORK_ID_TOKEN: Self =
        Self::from_static("PLAYSTATION_NETWORK_ID_TOKEN");

    const fn from_static(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }

    /// Makes a type from its raw value, for a type that this library does not know.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(Cow::Owned(value.into()))
    }

    /// Gives the raw value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The token response for a provisional account.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ProvisionalTokenResponse {
    /// The token type. It is `Bearer`.
    pub token_type: String,
    /// The access token.
    pub access_token: String,
    /// The number of seconds until the access token expires.
    pub expires_in: u64,
    /// The authorized scopes, separated by spaces.
    #[serde(default)]
    pub scope: String,
    /// The OpenID Connect ID token.
    #[serde(default)]
    pub id_token: String,
    /// The refresh token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// The authorized scopes as a list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    /// When the access token expires, in Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_s: Option<i64>,
}

impl ProvisionalTokenResponse {
    /// Gives a Bearer token to make a client for the provisional account.
    #[must_use]
    pub fn bearer_token(&self) -> Token {
        Token::bearer(&self.access_token)
    }
}

impl fmt::Debug for ProvisionalTokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvisionalTokenResponse")
            .field("token_type", &self.token_type)
            .field("access_token", &"<redacted>")
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .field("id_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("scopes", &self.scopes)
            .field("expires_at_s", &self.expires_at_s)
            .finish()
    }
}

/// The body of the requests that authenticate with external credentials.
#[derive(Clone, Serialize)]
struct ExternalCredentials {
    client_id: ApplicationId,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_secret"
    )]
    client_secret: Option<Zeroizing<String>>,
    external_auth_type: ExternalAuthType,
    #[serde(serialize_with = "serialize_secret")]
    external_auth_token: Zeroizing<String>,
}

fn serialize_secret<S: serde::Serializer>(
    secret: &Zeroizing<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(secret)
}

// Serde gives `serialize_with` a reference to the field type.
#[allow(clippy::ref_option)]
fn serialize_optional_secret<S: serde::Serializer>(
    secret: &Option<Zeroizing<String>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match secret {
        Some(secret) => serializer.serialize_str(secret),
        None => serializer.serialize_none(),
    }
}

impl fmt::Debug for ExternalCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalCredentials")
            .field("client_id", &self.client_id)
            .field("external_auth_type", &self.external_auth_type)
            .finish_non_exhaustive()
    }
}

/// `POST /partner-sdk/token`: gets an access token for a provisional account from an
/// external authentication token.
///
/// The request sends no `Authorization` header.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateProvisionalToken {
    client: Client,
    body: ExternalCredentials,
}

impl CreateProvisionalToken {
    /// Sets the client secret. Public clients do not send it.
    pub fn client_secret(mut self, secret: impl Into<String>) -> Self {
        self.body.client_secret = Some(Zeroizing::new(secret.into()));
        self
    }
}

impl Endpoint for CreateProvisionalToken {
    type Response = ProvisionalTokenResponse;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/partner-sdk/token",
            "/partner-sdk/token".to_owned(),
            MajorParameter::None,
        )
        .with_path_token()
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `POST /partner-sdk/token/bot`: gets an access token for a provisional account from the
/// user ID in your account system.
///
/// Discord makes the provisional account if it does not exist. The request fails with JSON
/// error code 530010 when the external user has a full Discord account.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateBotProvisionalToken {
    client: Client,
    body: BotProvisionalTokenBody,
}

#[derive(Clone, Debug, Serialize)]
struct BotProvisionalTokenBody {
    external_user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    provisional_user_id: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_global_name: Option<String>,
}

impl CreateBotProvisionalToken {
    /// Sets the display name of the provisional account, up to 32 characters.
    pub fn preferred_global_name(mut self, name: impl Into<String>) -> Self {
        self.body.preferred_global_name = Some(name.into());
        self
    }

    /// Sets the user ID of an existing provisional account.
    pub fn provisional_user_id(mut self, user_id: UserId) -> Self {
        self.body.provisional_user_id = Some(user_id);
        self
    }
}

impl Endpoint for CreateBotProvisionalToken {
    type Response = ProvisionalTokenResponse;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/partner-sdk/token/bot",
            "/partner-sdk/token/bot".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `POST /partner-sdk/provisional-accounts/unmerge`: separates a provisional account from
/// the Discord account that it was merged into, with external credentials.
///
/// The request sends no `Authorization` header.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UnmergeProvisionalAccount {
    client: Client,
    body: ExternalCredentials,
}

impl UnmergeProvisionalAccount {
    /// Sets the client secret. Public clients do not send it.
    pub fn client_secret(mut self, secret: impl Into<String>) -> Self {
        self.body.client_secret = Some(Zeroizing::new(secret.into()));
        self
    }
}

impl Endpoint for UnmergeProvisionalAccount {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/partner-sdk/provisional-accounts/unmerge",
            "/partner-sdk/provisional-accounts/unmerge".to_owned(),
            MajorParameter::None,
        )
        .with_path_token()
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `POST /partner-sdk/provisional-accounts/unmerge/bot`: separates a provisional account
/// from the Discord account that it was merged into, with the user ID in your account
/// system.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UnmergeBotProvisionalAccount {
    client: Client,
    external_user_id: String,
}

#[derive(Serialize)]
struct UnmergeBotBody<'a> {
    external_user_id: &'a str,
}

impl Endpoint for UnmergeBotProvisionalAccount {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/partner-sdk/provisional-accounts/unmerge/bot",
            "/partner-sdk/provisional-accounts/unmerge/bot".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&UnmergeBotBody {
            external_user_id: &self.external_user_id,
        })
    }
}

/// `PUT /partner-sdk/dms/{user_id_1}/{user_id_2}/messages/{message_id}/moderation-metadata`:
/// sets the moderation metadata of a DM message between two users.
///
/// Discord sends the metadata to active Social SDK clients as a message update.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UpdateDmMessageModerationMetadata {
    client: Client,
    user_ids: (UserId, UserId),
    message_id: MessageId,
    metadata: Metadata,
}

impl Endpoint for UpdateDmMessageModerationMetadata {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/partner-sdk/dms/{user_id_1}/{user_id_2}/messages/{message_id}/moderation-metadata",
            format!(
                "/partner-sdk/dms/{}/{}/messages/{}/moderation-metadata",
                self.user_ids.0, self.user_ids.1, self.message_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_moderation_metadata(&self.metadata))?;
        RequestBody::json(&self.metadata)
    }
}

super::into_future!(
    CreateProvisionalToken,
    CreateBotProvisionalToken,
    UnmergeProvisionalAccount,
    UnmergeBotProvisionalAccount,
    UpdateDmMessageModerationMetadata,
);

impl Client {
    /// Gets an access token for a provisional account from an external authentication token.
    pub fn create_provisional_token(
        &self,
        client_id: ApplicationId,
        external_auth_type: ExternalAuthType,
        external_auth_token: impl Into<String>,
    ) -> CreateProvisionalToken {
        CreateProvisionalToken {
            client: self.clone(),
            body: ExternalCredentials {
                client_id,
                client_secret: None,
                external_auth_type,
                external_auth_token: Zeroizing::new(external_auth_token.into()),
            },
        }
    }

    /// Gets an access token for a provisional account from the user ID in your account
    /// system.
    pub fn create_bot_provisional_token(
        &self,
        external_user_id: impl Into<String>,
    ) -> CreateBotProvisionalToken {
        CreateBotProvisionalToken {
            client: self.clone(),
            body: BotProvisionalTokenBody {
                external_user_id: external_user_id.into(),
                provisional_user_id: None,
                preferred_global_name: None,
            },
        }
    }

    /// Separates a provisional account from a Discord account, with external credentials.
    pub fn unmerge_provisional_account(
        &self,
        client_id: ApplicationId,
        external_auth_type: ExternalAuthType,
        external_auth_token: impl Into<String>,
    ) -> UnmergeProvisionalAccount {
        UnmergeProvisionalAccount {
            client: self.clone(),
            body: ExternalCredentials {
                client_id,
                client_secret: None,
                external_auth_type,
                external_auth_token: Zeroizing::new(external_auth_token.into()),
            },
        }
    }

    /// Separates a provisional account from a Discord account, with the user ID in your
    /// account system.
    pub fn unmerge_bot_provisional_account(
        &self,
        external_user_id: impl Into<String>,
    ) -> UnmergeBotProvisionalAccount {
        UnmergeBotProvisionalAccount {
            client: self.clone(),
            external_user_id: external_user_id.into(),
        }
    }

    /// Sets the moderation metadata of a DM message between two users (up to 5 keys).
    pub fn update_dm_message_moderation_metadata(
        &self,
        user_id_1: UserId,
        user_id_2: UserId,
        message_id: MessageId,
        metadata: Metadata,
    ) -> UpdateDmMessageModerationMetadata {
        UpdateDmMessageModerationMetadata {
            client: self.clone(),
            user_ids: (user_id_1, user_id_2),
            message_id,
            metadata,
        }
    }
}
