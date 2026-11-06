//! Typed modals.

use cordial_model::component::Component;
use cordial_model::interaction::{ModalResponse, ModalSubmitData};

use crate::command::ParseError;

/// A modal with typed fields. `#[derive(Modal)]` implements it.
///
/// Each field is a text input in a `Label`. A `String` field is required. An
/// `Option<String>` field is not required.
pub trait Modal: Sized {
    /// The modal title.
    const TITLE: &'static str;

    /// Gives the components of an empty modal.
    fn components() -> Vec<Component>;

    /// Gives the components with the values of `self` filled in.
    fn prefilled_components(&self) -> Vec<Component>;

    /// Parses the submitted data.
    ///
    /// # Errors
    ///
    /// Gives an error when a required field is absent.
    fn parse(data: &ModalSubmitData) -> Result<Self, ParseError>;

    /// Makes an empty modal with a custom ID.
    #[must_use]
    fn modal(custom_id: impl Into<String>) -> ModalResponse {
        ModalResponse::new(custom_id, Self::TITLE, Self::components())
    }

    /// Makes a modal with the values of `self` filled in, for example to edit a value.
    #[must_use]
    fn prefilled(&self, custom_id: impl Into<String>) -> ModalResponse {
        ModalResponse::new(custom_id, Self::TITLE, self.prefilled_components())
    }
}
