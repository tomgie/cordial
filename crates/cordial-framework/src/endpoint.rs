//! Interactions over HTTP: a handler for the `cordial-endpoint` crate.

use std::sync::Arc;
use std::time::Duration;

use cordial_endpoint::{Handler, Reply};
use cordial_model::gateway::event::{Event, InteractionCreate};
use cordial_model::interaction::Interaction;

use crate::context::Context;
use crate::router::Router;

/// The longest wait for the initial response. Discord closes the request after 3 seconds.
const RESPONSE_WAIT: Duration = Duration::from_secs(3);

/// Sends interactions from HTTP requests to the bot handlers.
///
/// The handlers answer with the interaction callback endpoint, as they do for gateway
/// interactions. The HTTP request ends with status 202 after that answer.
pub struct EndpointHandler<S> {
    context: Context<S>,
    router: Arc<Router<S>>,
}

impl<S> std::fmt::Debug for EndpointHandler<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EndpointHandler")
            .finish_non_exhaustive()
    }
}

impl<S: Send + Sync + 'static> EndpointHandler<S> {
    pub(crate) fn new(context: Context<S>, router: Arc<Router<S>>) -> Self {
        Self { context, router }
    }
}

impl<S: Send + Sync + 'static> Handler for EndpointHandler<S> {
    async fn interaction(&self, interaction: Interaction) -> Reply {
        // The same path as gateway interactions: collectors first, then the handlers.
        let event = Arc::new(Event::InteractionCreate(Box::new(InteractionCreate(
            interaction.clone(),
        ))));
        let tracker = self
            .router
            .route_interaction(&self.context, &event, &interaction);

        let mut acknowledged = tracker.subscribe();
        let wait = acknowledged.wait_for(|acknowledged| *acknowledged);
        if tokio::time::timeout(RESPONSE_WAIT, wait).await.is_err() {
            tracing::warn!("no response within 3 seconds to an HTTP interaction");
        }
        Reply::Acknowledged
    }
}
