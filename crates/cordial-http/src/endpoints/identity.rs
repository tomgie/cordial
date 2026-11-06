//! Application identity endpoints: game profiles of users from external account systems.
//!
//! All these endpoints use the bot token. The user must authorize the application with the
//! `application_identities.write` scope.

use cordial_model::id::{ApplicationId, UserId};
use cordial_model::lobby::{ApplicationIdentity, ApplicationIdentityProfile, ProfileData};
use cordial_model::validate::ValidationError;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route, encode_segment};

/// The maximum size of the serialized `data` object of a profile, in bytes.
pub const PROFILE_DATA_SIZE_LIMIT: usize = 10 * 1024;
/// The maximum number of dynamic fields in a profile.
pub const DYNAMIC_FIELD_COUNT_LIMIT: usize = 30;
/// The maximum number of characters in the username of a profile.
pub const PROFILE_USERNAME_LIMIT: usize = 1024;

/// A list of application identities.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationIdentities {
    /// The identities. The list is empty when no identity matches.
    #[serde(default)]
    pub identities: Vec<ApplicationIdentity>,
}

/// `PATCH /applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile`:
/// changes the game profile of an application identity.
///
/// The first update makes a profile-only identity with the provider type `NONE`. Discord
/// answers "201 Created" on the first update and "204 No Content" after that. The response
/// is `None` when the body is empty.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditApplicationIdentityProfile {
    client: Client,
    application_id: ApplicationId,
    user_id: UserId,
    provider_issued_user_id: String,
    body: EditProfileBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditProfileBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<ProfileData>,
}

impl EditApplicationIdentityProfile {
    /// Sets the username in the external system, up to 1024 characters.
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.body.username = Some(username.into());
        self
    }

    /// Sets the game statistics. They replace all current statistics.
    ///
    /// Always send the complete data. Discord does not merge it with the stored data.
    pub fn data(mut self, data: ProfileData) -> Self {
        self.body.data = Some(data);
        self
    }

    fn check_limits(&self) -> Result<(), ValidationError> {
        if let Some(username) = &self.body.username {
            let count = username.chars().count();
            if count > PROFILE_USERNAME_LIMIT {
                return Err(ValidationError::new(
                    "username",
                    format!("has {count} characters; the maximum is {PROFILE_USERNAME_LIMIT}"),
                ));
            }
        }
        if let Some(data) = &self.body.data {
            let dynamic_count = data.dynamic.as_ref().map_or(0, Vec::len);
            if dynamic_count > DYNAMIC_FIELD_COUNT_LIMIT {
                return Err(ValidationError::new(
                    "data.dynamic",
                    format!(
                        "has {dynamic_count} fields; the maximum is {DYNAMIC_FIELD_COUNT_LIMIT}"
                    ),
                ));
            }
            let size = serde_json::to_vec(data).map_or(0, |bytes| bytes.len());
            if size > PROFILE_DATA_SIZE_LIMIT {
                return Err(ValidationError::new(
                    "data",
                    format!("has {size} bytes; the maximum is {PROFILE_DATA_SIZE_LIMIT}"),
                ));
            }
        }
        Ok(())
    }
}

impl Endpoint for EditApplicationIdentityProfile {
    type Response = Option<ApplicationIdentityProfile>;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile",
            format!(
                "/applications/{}/users/{}/identities/{}/profile",
                self.application_id,
                self.user_id,
                encode_segment(&self.provider_issued_user_id)
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.check_limits())?;
        RequestBody::json(&self.body)
    }
}

/// `GET /applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile`:
/// the game profile of an application identity.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetApplicationIdentityProfile {
    client: Client,
    application_id: ApplicationId,
    user_id: UserId,
    provider_issued_user_id: String,
}

impl Endpoint for GetApplicationIdentityProfile {
    type Response = ApplicationIdentityProfile;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile",
            format!(
                "/applications/{}/users/{}/identities/{}/profile",
                self.application_id,
                self.user_id,
                encode_segment(&self.provider_issued_user_id)
            ),
            MajorParameter::None,
        )
    }
}

/// `GET /users/{user_id}/application-identities/{application_id}`: the application
/// identities of a user. The response has no profile data.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetUserApplicationIdentities {
    client: Client,
    user_id: UserId,
    application_id: ApplicationId,
}

impl Endpoint for GetUserApplicationIdentities {
    type Response = ApplicationIdentities;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/{user_id}/application-identities/{application_id}",
            format!(
                "/users/{}/application-identities/{}",
                self.user_id, self.application_id
            ),
            MajorParameter::None,
        )
    }
}

/// `GET /applications/{application_id}/application-identities/{provider_type}/{provider_issued_user_id}`:
/// the application identities of the user that has an external ID.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetApplicationIdentitiesByExternalId {
    client: Client,
    application_id: ApplicationId,
    provider_type: String,
    provider_issued_user_id: String,
    provider_id: Option<String>,
}

impl GetApplicationIdentitiesByExternalId {
    /// Sets the provider identifier that tells identities with the same type and user ID
    /// apart.
    pub fn provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = Some(provider_id.into());
        self
    }
}

impl Endpoint for GetApplicationIdentitiesByExternalId {
    type Response = ApplicationIdentities;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("provider_id", self.provider_id.as_deref());
        Route::new(
            Method::Get,
            "/applications/{application_id}/application-identities/{provider_type}/{provider_issued_user_id}",
            format!(
                "/applications/{}/application-identities/{}/{}{}",
                self.application_id,
                encode_segment(&self.provider_type),
                encode_segment(&self.provider_issued_user_id),
                query.finish()
            ),
            MajorParameter::None,
        )
    }
}

/// `POST /users/{user_id}/application-identities/{application_id}/{provider_type}/{provider_issued_user_id}/delete`:
/// deletes an application identity and its profile data.
///
/// Discord refuses to delete the last account-linking identity of the user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteApplicationIdentity {
    client: Client,
    user_id: UserId,
    application_id: ApplicationId,
    provider_type: String,
    provider_issued_user_id: String,
    provider_id: Option<String>,
}

#[derive(Serialize)]
struct DeleteApplicationIdentityBody<'a> {
    provider_id: &'a str,
}

impl DeleteApplicationIdentity {
    /// Sets the provider identifier that tells identities with the same type and user ID
    /// apart.
    pub fn provider_id(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = Some(provider_id.into());
        self
    }
}

impl Endpoint for DeleteApplicationIdentity {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/users/{user_id}/application-identities/{application_id}/{provider_type}/{provider_issued_user_id}/delete",
            format!(
                "/users/{}/application-identities/{}/{}/{}/delete",
                self.user_id,
                self.application_id,
                encode_segment(&self.provider_type),
                encode_segment(&self.provider_issued_user_id)
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        match &self.provider_id {
            Some(provider_id) => RequestBody::json(&DeleteApplicationIdentityBody { provider_id }),
            None => Ok(RequestBody::None),
        }
    }
}

super::into_future!(
    EditApplicationIdentityProfile,
    GetApplicationIdentityProfile,
    GetUserApplicationIdentities,
    GetApplicationIdentitiesByExternalId,
    DeleteApplicationIdentity,
);

impl Client {
    /// Changes the game profile of an application identity. The first update makes the
    /// identity.
    pub fn edit_application_identity_profile(
        &self,
        application_id: ApplicationId,
        user_id: UserId,
        provider_issued_user_id: impl Into<String>,
    ) -> EditApplicationIdentityProfile {
        EditApplicationIdentityProfile {
            client: self.clone(),
            application_id,
            user_id,
            provider_issued_user_id: provider_issued_user_id.into(),
            body: EditProfileBody::default(),
        }
    }

    /// Gets the game profile of an application identity.
    pub fn application_identity_profile(
        &self,
        application_id: ApplicationId,
        user_id: UserId,
        provider_issued_user_id: impl Into<String>,
    ) -> GetApplicationIdentityProfile {
        GetApplicationIdentityProfile {
            client: self.clone(),
            application_id,
            user_id,
            provider_issued_user_id: provider_issued_user_id.into(),
        }
    }

    /// Gets the application identities of a user.
    pub fn user_application_identities(
        &self,
        user_id: UserId,
        application_id: ApplicationId,
    ) -> GetUserApplicationIdentities {
        GetUserApplicationIdentities {
            client: self.clone(),
            user_id,
            application_id,
        }
    }

    /// Gets the application identities of the user that has an external ID.
    pub fn application_identities_by_external_id(
        &self,
        application_id: ApplicationId,
        provider_type: impl Into<String>,
        provider_issued_user_id: impl Into<String>,
    ) -> GetApplicationIdentitiesByExternalId {
        GetApplicationIdentitiesByExternalId {
            client: self.clone(),
            application_id,
            provider_type: provider_type.into(),
            provider_issued_user_id: provider_issued_user_id.into(),
            provider_id: None,
        }
    }

    /// Deletes an application identity and its profile data.
    pub fn delete_application_identity(
        &self,
        user_id: UserId,
        application_id: ApplicationId,
        provider_type: impl Into<String>,
        provider_issued_user_id: impl Into<String>,
    ) -> DeleteApplicationIdentity {
        DeleteApplicationIdentity {
            client: self.clone(),
            user_id,
            application_id,
            provider_type: provider_type.into(),
            provider_issued_user_id: provider_issued_user_id.into(),
            provider_id: None,
        }
    }
}
