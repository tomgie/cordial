use cordial_framework::{Choice, CommandGroup, Modal};

#[derive(Choice)]
enum Mixed {
    #[choice(value = 1)]
    One,
    #[choice(value = "two")]
    Two,
    Three,
}

#[derive(Modal)]
#[modal(title = "A title that is much too long for a Discord modal")]
struct Form {
    #[field(label = "Age")]
    age: u32,
    #[field(label = "Name", style = "large")]
    name: String,
}

#[derive(CommandGroup)]
enum Group {
    Missing,
    #[subcommand(description = "Named fields")]
    Named { value: String },
}

fn main() {}
