use cordial_framework::Command;

#[derive(Command)]
#[command(name = "Ban", description = "Ban a member")]
struct Uppercase;

#[derive(Command)]
#[command(name = "order", description = "Wrong option order")]
struct Order {
    #[option(description = "Optional")]
    first: Option<String>,
    #[option(description = "Required")]
    second: String,
}

#[derive(Command)]
#[command(name = "nodescription")]
struct NoDescription {
    value: String,
}

#[derive(Command)]
#[command(name = "limits", description = "Bad limits")]
struct Limits {
    #[option(description = "", min_value = 5, max_value = 1)]
    value: i64,
    #[option(description = "Text", max_length = 7000, colour = 1)]
    text: String,
}

fn main() {}
