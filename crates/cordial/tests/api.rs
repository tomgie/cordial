//! Compile-time checks of the public API.
//!
//! Errors must work with `?` across tasks, and handles must move between tasks. A change that
//! breaks one of these rules does not compile.

#![allow(dead_code)]

fn error<E: std::error::Error + Send + Sync + 'static>() {}

fn shared<T: Send + Sync + 'static>() {}

#[test]
fn errors_are_send_and_sync() {
    error::<cordial::model::id::ParseIdError>();
    error::<cordial::model::time::ParseTimestampError>();
    error::<cordial::model::validate::ValidationError>();

    error::<cordial::http::Error>();
    error::<cordial::http::ApiError>();
    error::<cordial::http::TransportError>();

    error::<cordial::gateway::CommandError>();
    error::<cordial::gateway::FatalError>();
    error::<cordial::gateway::ManagerError>();
    error::<cordial::gateway::ShardError>();
    error::<cordial::gateway::compression::DecompressError>();

    error::<cordial::cache::PermissionError>();

    error::<cordial::framework::Error>();
    error::<cordial::framework::HandlerError>();
    error::<cordial::framework::ParseError>();
    error::<cordial::framework::component::PatternError>();
    error::<cordial::framework::component::ParamError>();
    error::<cordial::framework::interaction::ResponseError>();
    error::<cordial::framework::interaction::MissingValue>();
    error::<cordial::framework::sync::PlanError>();
    error::<cordial::framework::sync::SyncError>();
}

#[test]
fn handles_are_send_and_sync() {
    shared::<cordial::http::Client>();
    shared::<cordial::gateway::ShardHandle>();
    shared::<cordial::gateway::ShardManager>();
    shared::<cordial::cache::Cache>();
    shared::<cordial::framework::Context<()>>();
    shared::<cordial::framework::InteractionContext<()>>();
    shared::<cordial::framework::ComponentContext<()>>();
    shared::<cordial::framework::Bot<()>>();
}

#[cfg(feature = "endpoint")]
#[test]
fn endpoint_types_are_send_and_sync() {
    error::<cordial::endpoint::VerifyError>();
    shared::<cordial::endpoint::Endpoint<cordial::framework::endpoint::EndpointHandler<()>>>();
}

#[cfg(feature = "voice")]
#[test]
fn voice_types_are_send_and_sync() {
    error::<cordial::voice::AudioError>();
    error::<cordial::voice::CallError>();
    error::<cordial::voice::JoinError>();
    error::<cordial::voice::crypto::CryptoError>();
    error::<cordial::voice::packet::PacketError>();
    error::<cordial::voice::e2ee::E2eeError>();

    shared::<cordial::voice::Call>();
    shared::<cordial::voice::VoiceManager>();
    shared::<cordial::voice::TrackHandle>();
}
