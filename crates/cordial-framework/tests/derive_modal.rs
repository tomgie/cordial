//! Tests of `#[derive(Modal)]`: the components, the prefilled values, and the parser.

use cordial_framework::Modal;
use cordial_framework::command::ParseErrorKind;
use cordial_model::interaction::ModalSubmitData;
use serde_json::{Value, json};

#[derive(Modal, Debug, PartialEq)]
#[modal(title = "Send feedback")]
struct Feedback {
    #[field(label = "Subject", placeholder = "A short summary", max_length = 80)]
    subject: String,
    #[field(
        label = "Details",
        description = "Tell us what happened",
        style = "paragraph",
        min_length = 10,
        max_length = 2000,
        custom_id = "feedback-details"
    )]
    details: Option<String>,
    #[field(label = "Contact")]
    contact: Option<String>,
}

fn components_json(feedback: Option<&Feedback>) -> Value {
    let components = match feedback {
        Some(feedback) => feedback.prefilled_components(),
        None => Feedback::components(),
    };
    serde_json::to_value(components).unwrap()
}

fn submit_data(components: Value) -> ModalSubmitData {
    let mut data = json!({ "custom_id": "feedback" });
    data["components"] = components;
    serde_json::from_value(data).unwrap()
}

fn submitted(id: i32, custom_id: &str, value: &str) -> Value {
    json!({
        "type": 18,
        "id": id,
        "component": { "type": 4, "id": id + 1, "custom_id": custom_id, "value": value }
    })
}

#[test]
fn modal_components() {
    assert_eq!(Feedback::TITLE, "Send feedback");

    let expected = json!([
        {
            "type": 18,
            "label": "Subject",
            "component": {
                "type": 4,
                "custom_id": "subject",
                "style": 1,
                "max_length": 80,
                "placeholder": "A short summary"
            }
        },
        {
            "type": 18,
            "label": "Details",
            "description": "Tell us what happened",
            "component": {
                "type": 4,
                "custom_id": "feedback-details",
                "style": 2,
                "required": false,
                "min_length": 10,
                "max_length": 2000
            }
        },
        {
            "type": 18,
            "label": "Contact",
            "component": { "type": 4, "custom_id": "contact", "style": 1, "required": false }
        }
    ]);
    assert_eq!(components_json(None), expected);
}

#[test]
fn modal_response() {
    let response = serde_json::to_value(Feedback::modal("feedback")).unwrap();
    assert_eq!(response["custom_id"], "feedback");
    assert_eq!(response["title"], "Send feedback");
    assert_eq!(response["components"], components_json(None));
}

#[test]
fn prefilled_components() {
    let feedback = Feedback {
        subject: "Bug".to_owned(),
        details: Some("The bot does not start.".to_owned()),
        contact: Some(String::new()),
    };
    let components = components_json(Some(&feedback));

    assert_eq!(components[0]["component"]["value"], "Bug");
    assert_eq!(
        components[1]["component"]["value"],
        "The bot does not start."
    );
    assert!(components[2]["component"].get("value").is_none());

    let mut expected = components_json(None);
    expected[0]["component"]["value"] = json!("Bug");
    expected[1]["component"]["value"] = json!("The bot does not start.");
    assert_eq!(components, expected);
}

#[test]
fn parse_submit() {
    let data = submit_data(json!([
        submitted(1, "subject", "Bug"),
        submitted(3, "feedback-details", "The bot does not start."),
        submitted(5, "contact", "")
    ]));
    let expected = Feedback {
        subject: "Bug".to_owned(),
        details: Some("The bot does not start.".to_owned()),
        contact: None,
    };
    assert_eq!(Feedback::parse(&data).unwrap(), expected);

    let data = submit_data(json!([submitted(1, "subject", "Idea")]));
    let expected = Feedback {
        subject: "Idea".to_owned(),
        details: None,
        contact: None,
    };
    assert_eq!(Feedback::parse(&data).unwrap(), expected);
}

#[test]
fn parse_missing_field() {
    let data = submit_data(json!([submitted(3, "feedback-details", "Text")]));
    let error = Feedback::parse(&data).unwrap_err();
    assert_eq!(error.path, ["subject"]);
    assert_eq!(error.kind, ParseErrorKind::MissingField);
}
