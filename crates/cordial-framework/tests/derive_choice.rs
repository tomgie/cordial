//! Tests of `#[derive(Choice)]`: string, integer, and number choices.

use cordial_framework::command::ParseErrorKind;
use cordial_framework::{Choice, Command, CommandArg};
use cordial_model::command::CommandOptionType;
use cordial_model::interaction::{CommandData, CommandOptionValue};
use serde_json::{Value, json};

#[derive(Choice, Debug, PartialEq, Clone, Copy)]
enum Animal {
    Cat,
    #[choice(name = "Big dog")]
    BigDog,
    #[choice(name = "Fish", value = "goldfish")]
    Fish,
}

#[derive(Choice, Debug, PartialEq)]
enum Speed {
    #[choice(name = "Slow", value = 1)]
    Slow,
    #[choice(name = "Fast", value = 10)]
    Fast,
    #[choice(name = "Reverse", value = -1)]
    Reverse,
}

#[derive(Choice, Debug, PartialEq)]
enum Ratio {
    #[choice(name = "Half", value = 0.5)]
    Half,
    #[choice(name = "Double", value = 2.0)]
    Double,
}

#[derive(Command, Debug)]
#[command(name = "pet", description = "Show a pet")]
struct Pet {
    #[option(description = "The animal")]
    animal: Animal,
    #[option(description = "The speed")]
    speed: Option<Speed>,
    #[option(description = "The ratio")]
    ratio: Option<Ratio>,
}

fn pet_data(options: Value) -> CommandData {
    let mut data = json!({ "id": "100000000000000001", "name": "pet", "type": 1 });
    data["options"] = options;
    serde_json::from_value(data).unwrap()
}

#[test]
fn choice_definitions() {
    let expected = json!({
        "type": 1,
        "name": "pet",
        "description": "Show a pet",
        "options": [
            {
                "type": 3,
                "name": "animal",
                "description": "The animal",
                "required": true,
                "choices": [
                    { "name": "Cat", "value": "cat" },
                    { "name": "Big dog", "value": "big_dog" },
                    { "name": "Fish", "value": "goldfish" }
                ]
            },
            {
                "type": 4,
                "name": "speed",
                "description": "The speed",
                "choices": [
                    { "name": "Slow", "value": 1 },
                    { "name": "Fast", "value": 10 },
                    { "name": "Reverse", "value": -1 }
                ]
            },
            {
                "type": 10,
                "name": "ratio",
                "description": "The ratio",
                "choices": [
                    { "name": "Half", "value": 0.5 },
                    { "name": "Double", "value": 2.0 }
                ]
            }
        ]
    });
    assert_eq!(serde_json::to_value(Pet::definition()).unwrap(), expected);
}

#[test]
fn parse_choices() {
    let data = pet_data(json!([
        { "name": "animal", "type": 3, "value": "big_dog" },
        { "name": "speed", "type": 4, "value": -1 },
        { "name": "ratio", "type": 10, "value": 0.5 }
    ]));
    let pet = Pet::parse(&data).unwrap();
    assert_eq!(pet.animal, Animal::BigDog);
    assert_eq!(pet.speed, Some(Speed::Reverse));
    assert_eq!(pet.ratio, Some(Ratio::Half));

    // Discord can send a whole number as an integer for a number option.
    let data = pet_data(json!([
        { "name": "animal", "type": 3, "value": "goldfish" },
        { "name": "ratio", "type": 10, "value": 2 }
    ]));
    let pet = Pet::parse(&data).unwrap();
    assert_eq!(pet.animal, Animal::Fish);
    assert_eq!(pet.speed, None);
    assert_eq!(pet.ratio, Some(Ratio::Double));
}

#[test]
fn parse_errors() {
    let data = pet_data(json!([{ "name": "animal", "type": 3, "value": "horse" }]));
    let error = Pet::parse(&data).unwrap_err();
    assert_eq!(error.path, ["animal"]);
    assert_eq!(
        error.kind,
        ParseErrorKind::UnknownChoice("horse".to_owned())
    );

    let data = pet_data(json!([
        { "name": "animal", "type": 3, "value": "cat" },
        { "name": "speed", "type": 4, "value": 5 }
    ]));
    let error = Pet::parse(&data).unwrap_err();
    assert_eq!(error.path, ["speed"]);
    assert_eq!(error.kind, ParseErrorKind::UnknownChoice("5".to_owned()));

    let value = CommandOptionValue::Integer(1);
    let error = Animal::parse(&value, None).unwrap_err();
    assert_eq!(
        error.kind,
        ParseErrorKind::WrongType {
            expected: CommandOptionType::String
        }
    );
}

#[test]
fn choice_methods() {
    assert_eq!(Animal::BigDog.choice_name(), "Big dog");
    assert_eq!(Animal::BigDog.choice_value(), "big_dog");
    assert_eq!(Speed::Fast.choice_value(), 10);
    assert!((Ratio::Half.choice_value() - 0.5).abs() < f64::EPSILON);
}
