use cordial_framework::Command;

#[derive(Command)]
#[command(name = "ban", description = "Ban", default_member_permissions = "BAN_MEMBRES")]
struct Ban;

#[derive(Command)]
#[command(name = "where", description = "Where", contexts = "guild, server")]
struct Where;

#[derive(Command)]
#[command(name = "Wrong target", kind = "message")]
struct WrongTarget {
    target: cordial_framework::command::ResolvedUser,
}

fn main() {}
