//! OAuth2: authorization URLs, the token exchange, the token revocation, and the OAuth2
//! information endpoints.
//!
//! The token and revocation endpoints send no `Authorization` header. The request body has
//! `client_id` and `client_secret` as form fields, as the Discord docs permit. A client
//! without a token can send these requests:
//!
//! ```no_run
//! # async fn run() -> Result<(), cordial_http::Error> {
//! use cordial_http::Client;
//! use cordial_model::id::ApplicationId;
//!
//! let client = Client::builder().build();
//! let token = client
//!     .exchange_code(ApplicationId::new_const(1), "SECRET", "CODE", "https://example.com")
//!     .code_verifier("VERIFIER")
//!     .await?;
//! let user_client = Client::new(token.bearer_token());
//! # Ok(())
//! # }
//! ```
//!
//! [`GetCurrentAuthorization`] and [`GetOAuth2UserInfo`] need a client with a Bearer token.

use std::fmt;

use cordial_model::application::{Application, ApplicationIntegrationType};
use cordial_model::guild::Guild;
use cordial_model::id::{ApplicationId, GuildId};
use cordial_model::oauth2::Scope;
use cordial_model::user::User;
use cordial_model::webhook::Webhook;
use cordial_model::{Permissions, Timestamp};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody};
use crate::route::{MajorParameter, Method, Query, Route};
use crate::token::Token;

/// The base authorization URL.
pub const AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";

/// The issuer of Discord authorization responses: the expected `iss` value of a redirect.
pub const ISSUER: &str = "https://discord.com";

/// Tells if the `iss` value of an authorization redirect identifies Discord.
///
/// Check the value before you exchange the code (RFC 9207). This prevents mix-up attacks
/// when an application supports more than one OAuth2 provider. The comparison is exact.
#[must_use]
pub fn is_discord_issuer(iss: &str) -> bool {
    iss == ISSUER
}

/// What the authorization screen does when the user already authorized the scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Prompt {
    /// Ask the user to approve again.
    Consent,
    /// Skip the screen and redirect at once. The `bot` and `webhook.incoming` scopes always
    /// show the screen.
    None,
}

impl Prompt {
    /// Gives the value of the `prompt` parameter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Consent => "consent",
            Self::None => "none",
        }
    }
}

fn join_scopes(scopes: &[Scope]) -> String {
    scopes
        .iter()
        .map(Scope::as_str)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Builds the URL of the authorization code grant.
///
/// ```
/// use cordial_http::endpoints::oauth2::{AuthorizationUrl, Prompt};
/// use cordial_model::id::ApplicationId;
/// use cordial_model::oauth2::Scope;
///
/// let url = AuthorizationUrl::new(ApplicationId::new_const(157730590492196864))
///     .scopes([Scope::Identify, Scope::GuildsJoin])
///     .state("15773059ghq9183habn")
///     .redirect_uri("https://nicememe.website")
///     .prompt(Prompt::Consent)
///     .build();
/// assert!(url.starts_with("https://discord.com/oauth2/authorize?response_type=code&"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct AuthorizationUrl {
    client_id: ApplicationId,
    scopes: Vec<Scope>,
    state: Option<String>,
    redirect_uri: Option<String>,
    prompt: Option<Prompt>,
    integration_type: Option<ApplicationIntegrationType>,
    code_challenge: Option<String>,
    permissions: Option<Permissions>,
    guild_id: Option<GuildId>,
}

impl AuthorizationUrl {
    /// Starts an authorization URL for an application.
    pub fn new(client_id: ApplicationId) -> Self {
        Self {
            client_id,
            scopes: Vec::new(),
            state: None,
            redirect_uri: None,
            prompt: None,
            integration_type: None,
            code_challenge: None,
            permissions: None,
            guild_id: None,
        }
    }

    /// Sets the scopes.
    pub fn scopes(mut self, scopes: impl IntoIterator<Item = Scope>) -> Self {
        self.scopes = scopes.into_iter().collect();
        self
    }

    /// Sets the state: a value that binds the request to the session of the user.
    ///
    /// Compare it with the `state` of the redirect. This prevents cross-site request forgery.
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Sets the redirect URI. It must be one of the registered redirect URIs.
    pub fn redirect_uri(mut self, redirect_uri: impl Into<String>) -> Self {
        self.redirect_uri = Some(redirect_uri.into());
        self
    }

    /// Sets what the screen does when the user already authorized the scopes.
    pub fn prompt(mut self, prompt: Prompt) -> Self {
        self.prompt = Some(prompt);
        self
    }

    /// Sets the installation context. It applies only with the `applications.commands`
    /// scope.
    pub fn integration_type(mut self, integration_type: ApplicationIntegrationType) -> Self {
        self.integration_type = Some(integration_type);
        self
    }

    /// Sets the PKCE code challenge with the method `S256`.
    ///
    /// The challenge is the base64url text (no padding) of the SHA-256 hash of the code
    /// verifier. Calculate it yourself, and send the verifier with
    /// [`ExchangeToken::code_verifier`].
    pub fn code_challenge(mut self, challenge: impl Into<String>) -> Self {
        self.code_challenge = Some(challenge.into());
        self
    }

    /// Sets the bot permissions, for the `bot` scope with other scopes.
    pub fn permissions(mut self, permissions: Permissions) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Selects a guild in the guild list, for the `bot` scope with other scopes.
    pub fn guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Gives the URL.
    #[must_use]
    pub fn build(&self) -> String {
        let mut query = Query::new();
        query.push("response_type", "code");
        query.push("client_id", self.client_id);
        if !self.scopes.is_empty() {
            query.push("scope", join_scopes(&self.scopes));
        }
        query.push_opt("state", self.state.as_deref());
        query.push_opt("redirect_uri", self.redirect_uri.as_deref());
        query.push_opt("prompt", self.prompt.map(Prompt::as_str));
        query.push_opt("integration_type", self.integration_type);
        query.push_opt("permissions", self.permissions.map(Permissions::bits));
        query.push_opt("guild_id", self.guild_id);
        if let Some(challenge) = &self.code_challenge {
            query.push("code_challenge", challenge);
            query.push("code_challenge_method", "S256");
        }
        format!("{AUTHORIZE_URL}{}", query.finish())
    }
}

impl fmt::Display for AuthorizationUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.build())
    }
}

/// Builds the URL of the bot authorization flow, which adds a bot to a guild.
///
/// The default scope is `bot`. Without scopes, Discord uses the default install settings of
/// the application.
///
/// ```
/// use cordial_http::endpoints::oauth2::BotAuthorizationUrl;
/// use cordial_model::Permissions;
/// use cordial_model::id::ApplicationId;
///
/// let url = BotAuthorizationUrl::new(ApplicationId::new_const(157730590492196864))
///     .permissions(Permissions::from_bits_retain(1))
///     .build();
/// assert_eq!(
///     url,
///     "https://discord.com/oauth2/authorize?client_id=157730590492196864&scope=bot&permissions=1"
/// );
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct BotAuthorizationUrl {
    client_id: ApplicationId,
    scopes: Vec<Scope>,
    permissions: Option<Permissions>,
    guild_id: Option<GuildId>,
    disable_guild_select: Option<bool>,
    integration_type: Option<ApplicationIntegrationType>,
}

impl BotAuthorizationUrl {
    /// Starts a bot authorization URL for an application, with the `bot` scope.
    pub fn new(client_id: ApplicationId) -> Self {
        Self {
            client_id,
            scopes: vec![Scope::Bot],
            permissions: None,
            guild_id: None,
            disable_guild_select: None,
            integration_type: None,
        }
    }

    /// Sets the scopes. An empty list uses the default install settings of the application.
    pub fn scopes(mut self, scopes: impl IntoIterator<Item = Scope>) -> Self {
        self.scopes = scopes.into_iter().collect();
        self
    }

    /// Sets the permissions of the bot.
    pub fn permissions(mut self, permissions: Permissions) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Selects a guild in the guild list.
    pub fn guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Prevents the user from selecting a different guild. Use it with [`guild`](Self::guild).
    pub fn disable_guild_select(mut self, disable: bool) -> Self {
        self.disable_guild_select = Some(disable);
        self
    }

    /// Sets the installation context.
    pub fn integration_type(mut self, integration_type: ApplicationIntegrationType) -> Self {
        self.integration_type = Some(integration_type);
        self
    }

    /// Gives the URL.
    #[must_use]
    pub fn build(&self) -> String {
        let mut query = Query::new();
        query.push("client_id", self.client_id);
        if !self.scopes.is_empty() {
            query.push("scope", join_scopes(&self.scopes));
        }
        query.push_opt("permissions", self.permissions.map(Permissions::bits));
        query.push_opt("guild_id", self.guild_id);
        query.push_opt("disable_guild_select", self.disable_guild_select);
        query.push_opt("integration_type", self.integration_type);
        format!("{AUTHORIZE_URL}{}", query.finish())
    }
}

impl fmt::Display for BotAuthorizationUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.build())
    }
}

/// A secret text. `Debug` does not show it, and the memory is zeroed on drop.
#[derive(Clone)]
struct Secret(Zeroizing<String>);

impl Secret {
    fn new(text: impl Into<String>) -> Self {
        Self(Zeroizing::new(text.into()))
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

/// A form body (`application/x-www-form-urlencoded`).
struct Form(Zeroizing<String>);

impl Form {
    fn new() -> Self {
        Self(Zeroizing::new(String::new()))
    }

    fn push(&mut self, name: &str, value: &str) {
        if !self.0.is_empty() {
            self.0.push('&');
        }
        self.0.push_str(name);
        self.0.push('=');
        let encoded = percent_encoding::utf8_percent_encode(value, crate::QUERY_ENCODE_SET);
        for part in encoded {
            self.0.push_str(part);
        }
    }

    fn finish(self) -> RequestBody {
        RequestBody::Form(self.0.as_bytes().to_vec().into())
    }
}

/// The access token response of the token endpoint.
#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AccessTokenResponse {
    /// The access token.
    pub access_token: String,
    /// The token type. It is `Bearer`.
    pub token_type: String,
    /// The number of seconds until the access token expires.
    pub expires_in: u64,
    /// The refresh token. The client credentials grant gives none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// The authorized scopes, separated by spaces.
    #[serde(default)]
    pub scope: String,
    /// The OpenID Connect ID token, with the `openid` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    /// The guild that the bot joined, in the advanced bot authorization flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild: Option<Box<Guild>>,
    /// The new webhook, in the `webhook.incoming` flow. Keep its ID and token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<Box<Webhook>>,
}

impl AccessTokenResponse {
    /// Gives the authorized scopes.
    pub fn scopes(&self) -> impl Iterator<Item = Scope> + '_ {
        self.scope.split_whitespace().map(Scope::from_value)
    }

    /// Gives a Bearer token to make a client for the user.
    #[must_use]
    pub fn bearer_token(&self) -> Token {
        Token::bearer(&self.access_token)
    }
}

impl fmt::Debug for AccessTokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccessTokenResponse")
            .field("access_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("scope", &self.scope)
            .field("id_token", &self.id_token.as_ref().map(|_| "<redacted>"))
            .field("guild", &self.guild.as_ref().map(|guild| guild.id))
            .field("webhook", &self.webhook.as_ref().map(|webhook| webhook.id))
            .finish()
    }
}

#[derive(Clone, Debug)]
enum Grant {
    AuthorizationCode {
        code: Secret,
        redirect_uri: String,
        code_verifier: Option<Secret>,
    },
    RefreshToken {
        refresh_token: Secret,
    },
    ClientCredentials {
        scopes: Vec<Scope>,
    },
}

/// `POST /oauth2/token`: gets an access token.
///
/// The request sends no `Authorization` header. The body has the client ID and the client
/// secret.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ExchangeToken {
    client: Client,
    client_id: ApplicationId,
    client_secret: Secret,
    grant: Grant,
}

impl ExchangeToken {
    /// Sets the PKCE code verifier. Only for the authorization code grant.
    pub fn code_verifier(mut self, verifier: impl Into<String>) -> Self {
        if let Grant::AuthorizationCode { code_verifier, .. } = &mut self.grant {
            *code_verifier = Some(Secret::new(verifier));
        }
        self
    }

    /// Sets the scopes. Only for the client credentials grant.
    pub fn scopes(mut self, new_scopes: impl IntoIterator<Item = Scope>) -> Self {
        if let Grant::ClientCredentials { scopes } = &mut self.grant {
            *scopes = new_scopes.into_iter().collect();
        }
        self
    }
}

impl Endpoint for ExchangeToken {
    type Response = AccessTokenResponse;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/oauth2/token",
            "/oauth2/token".to_owned(),
            MajorParameter::None,
        )
        .with_path_token()
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        let mut form = Form::new();
        match &self.grant {
            Grant::AuthorizationCode {
                code,
                redirect_uri,
                code_verifier,
            } => {
                form.push("grant_type", "authorization_code");
                form.push("code", &code.0);
                form.push("redirect_uri", redirect_uri);
                if let Some(verifier) = code_verifier {
                    form.push("code_verifier", &verifier.0);
                }
            }
            Grant::RefreshToken { refresh_token } => {
                form.push("grant_type", "refresh_token");
                form.push("refresh_token", &refresh_token.0);
            }
            Grant::ClientCredentials { scopes } => {
                form.push("grant_type", "client_credentials");
                if !scopes.is_empty() {
                    form.push("scope", &join_scopes(scopes));
                }
            }
        }
        form.push("client_id", &self.client_id.to_string());
        form.push("client_secret", &self.client_secret.0);
        Ok(form.finish())
    }
}

/// The type of a token to revoke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenTypeHint {
    /// An access token.
    AccessToken,
    /// A refresh token.
    RefreshToken,
}

impl TokenTypeHint {
    /// Gives the value of the `token_type_hint` parameter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AccessToken => "access_token",
            Self::RefreshToken => "refresh_token",
        }
    }
}

/// `POST /oauth2/token/revoke`: revokes an access token or a refresh token.
///
/// Discord revokes all access and refresh tokens of the authorization, whatever the token
/// and the hint are.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RevokeToken {
    client: Client,
    client_id: ApplicationId,
    client_secret: Secret,
    token: Secret,
    token_type_hint: Option<TokenTypeHint>,
}

impl RevokeToken {
    /// Tells Discord the type of the token.
    pub fn token_type_hint(mut self, hint: TokenTypeHint) -> Self {
        self.token_type_hint = Some(hint);
        self
    }
}

impl Endpoint for RevokeToken {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/oauth2/token/revoke",
            "/oauth2/token/revoke".to_owned(),
            MajorParameter::None,
        )
        .with_path_token()
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        let mut form = Form::new();
        form.push("token", &self.token.0);
        if let Some(hint) = self.token_type_hint {
            form.push("token_type_hint", hint.as_str());
        }
        form.push("client_id", &self.client_id.to_string());
        form.push("client_secret", &self.client_secret.0);
        Ok(form.finish())
    }

    fn retry_safe(&self) -> bool {
        // A second revocation has no other effect.
        true
    }
}

/// `GET /oauth2/applications/@me`: the application of the bot.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentBotApplication {
    client: Client,
}

impl Endpoint for GetCurrentBotApplication {
    type Response = Application;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/oauth2/applications/@me",
            "/oauth2/applications/@me".to_owned(),
            MajorParameter::None,
        )
    }
}

/// Information about the current OAuth2 authorization.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AuthorizationInformation {
    /// The application (partial).
    pub application: Application,
    /// The scopes that the user authorized.
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// When the access token expires.
    pub expires: Timestamp,
    /// The user. Only with the `identify` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
}

/// `GET /oauth2/@me`: information about the current authorization.
///
/// This endpoint needs a Bearer token.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentAuthorization {
    client: Client,
}

impl Endpoint for GetCurrentAuthorization {
    type Response = AuthorizationInformation;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/oauth2/@me",
            "/oauth2/@me".to_owned(),
            MajorParameter::None,
        )
    }
}

/// A JSON Web Key Set: the public keys that sign Discord ID tokens.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct JsonWebKeySet {
    /// The keys.
    #[serde(default)]
    pub keys: Vec<JsonWebKey>,
}

/// A public key in the JSON Web Key format (RFC 7517).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct JsonWebKey {
    /// The key type, for example `RSA`.
    pub kty: String,
    /// The use of the key, for example `sig`.
    #[serde(rename = "use")]
    pub key_use: String,
    /// The key ID.
    pub kid: String,
    /// The RSA modulus, in base64url.
    pub n: String,
    /// The RSA exponent, in base64url.
    pub e: String,
    /// The algorithm, for example `RS256`.
    pub alg: String,
}

/// `GET /oauth2/keys`: the public keys that sign ID tokens.
///
/// The request sends no `Authorization` header. The OpenAPI specification is the only
/// source of this endpoint.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetOAuth2Keys {
    client: Client,
}

impl Endpoint for GetOAuth2Keys {
    type Response = JsonWebKeySet;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/oauth2/keys",
            "/oauth2/keys".to_owned(),
            MajorParameter::None,
        )
        .with_path_token()
    }
}

/// The OpenID Connect claims of the current user.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct OpenIdUserInfo {
    /// The subject: the user ID.
    pub sub: String,
    /// The email address. Only with the `email` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Tells if the email address is verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    /// The username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    /// The display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    /// The URL of the avatar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// The locale of the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

/// `GET /oauth2/userinfo`: the OpenID Connect claims of the current user.
///
/// This endpoint needs a Bearer token with the `openid` scope. The OpenAPI specification is
/// the only source of this endpoint.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetOAuth2UserInfo {
    client: Client,
}

impl Endpoint for GetOAuth2UserInfo {
    type Response = OpenIdUserInfo;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/oauth2/userinfo",
            "/oauth2/userinfo".to_owned(),
            MajorParameter::None,
        )
    }
}

super::into_future!(
    ExchangeToken,
    RevokeToken,
    GetCurrentBotApplication,
    GetCurrentAuthorization,
    GetOAuth2Keys,
    GetOAuth2UserInfo,
);

impl Client {
    /// Exchanges an authorization code for an access token.
    ///
    /// Use the same `redirect_uri` as in the authorization URL. Check the `state` and the
    /// `iss` of the redirect first (see [`is_discord_issuer`]).
    pub fn exchange_code(
        &self,
        client_id: ApplicationId,
        client_secret: impl Into<String>,
        code: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> ExchangeToken {
        ExchangeToken {
            client: self.clone(),
            client_id,
            client_secret: Secret::new(client_secret),
            grant: Grant::AuthorizationCode {
                code: Secret::new(code),
                redirect_uri: redirect_uri.into(),
                code_verifier: None,
            },
        }
    }

    /// Exchanges a refresh token for a new access token.
    pub fn refresh_access_token(
        &self,
        client_id: ApplicationId,
        client_secret: impl Into<String>,
        refresh_token: impl Into<String>,
    ) -> ExchangeToken {
        ExchangeToken {
            client: self.clone(),
            client_id,
            client_secret: Secret::new(client_secret),
            grant: Grant::RefreshToken {
                refresh_token: Secret::new(refresh_token),
            },
        }
    }

    /// Gets an access token for the owner of the application (client credentials grant).
    ///
    /// The response has no refresh token. Team applications can use only the `identify`
    /// and `applications.commands.update` scopes.
    pub fn exchange_client_credentials(
        &self,
        client_id: ApplicationId,
        client_secret: impl Into<String>,
    ) -> ExchangeToken {
        ExchangeToken {
            client: self.clone(),
            client_id,
            client_secret: Secret::new(client_secret),
            grant: Grant::ClientCredentials { scopes: Vec::new() },
        }
    }

    /// Revokes an access token or a refresh token, and all tokens of its authorization.
    pub fn revoke_token(
        &self,
        client_id: ApplicationId,
        client_secret: impl Into<String>,
        token: impl Into<String>,
    ) -> RevokeToken {
        RevokeToken {
            client: self.clone(),
            client_id,
            client_secret: Secret::new(client_secret),
            token: Secret::new(token),
            token_type_hint: None,
        }
    }

    /// Gets the application of the bot.
    pub fn current_bot_application(&self) -> GetCurrentBotApplication {
        GetCurrentBotApplication {
            client: self.clone(),
        }
    }

    /// Gets information about the current authorization. Needs a Bearer token.
    pub fn current_authorization(&self) -> GetCurrentAuthorization {
        GetCurrentAuthorization {
            client: self.clone(),
        }
    }

    /// Gets the public keys that sign ID tokens.
    pub fn oauth2_keys(&self) -> GetOAuth2Keys {
        GetOAuth2Keys {
            client: self.clone(),
        }
    }

    /// Gets the OpenID Connect claims of the current user. Needs a Bearer token with the
    /// `openid` scope.
    pub fn oauth2_user_info(&self) -> GetOAuth2UserInfo {
        GetOAuth2UserInfo {
            client: self.clone(),
        }
    }
}
