//! The types that one command option can have.

use cordial_model::channel::Channel;
use cordial_model::command::{CommandOption, CommandOptionType, NumberLimit};
use cordial_model::guild::{PartialMember, Role};
use cordial_model::id::{ChannelId, GenericId, RoleId, UserId};
use cordial_model::interaction::{CommandOptionValue, Resolved};
use cordial_model::message::Attachment;
use cordial_model::user::User;

use super::{ParseError, ParseErrorKind};

/// The largest integer that Discord accepts in an integer option: 2^53 - 1.
const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

/// The type of one command option.
///
/// Use `Option<T>` in a derived struct for an option that is not required.
pub trait CommandArg: Sized {
    /// The option type.
    const KIND: CommandOptionType;

    /// Adds the limits of the Rust type to the option definition, for example a minimum of 0
    /// for unsigned integers.
    #[must_use]
    fn configure(option: CommandOption) -> CommandOption {
        option
    }

    /// Parses the value.
    ///
    /// # Errors
    ///
    /// Gives an error when the value has the wrong type or is out of range.
    fn parse(value: &CommandOptionValue, resolved: Option<&Resolved>) -> Result<Self, ParseError>;
}

fn wrong_type(expected: CommandOptionType) -> ParseError {
    ParseError::new(ParseErrorKind::WrongType { expected })
}

impl CommandArg for String {
    const KIND: CommandOptionType = CommandOptionType::String;

    fn parse(value: &CommandOptionValue, _: Option<&Resolved>) -> Result<Self, ParseError> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| wrong_type(Self::KIND))
    }
}

impl CommandArg for bool {
    const KIND: CommandOptionType = CommandOptionType::Boolean;

    fn parse(value: &CommandOptionValue, _: Option<&Resolved>) -> Result<Self, ParseError> {
        value.as_bool().ok_or_else(|| wrong_type(Self::KIND))
    }
}

impl CommandArg for f64 {
    const KIND: CommandOptionType = CommandOptionType::Number;

    fn parse(value: &CommandOptionValue, _: Option<&Resolved>) -> Result<Self, ParseError> {
        value.as_f64().ok_or_else(|| wrong_type(Self::KIND))
    }
}

impl CommandArg for i64 {
    const KIND: CommandOptionType = CommandOptionType::Integer;

    fn parse(value: &CommandOptionValue, _: Option<&Resolved>) -> Result<Self, ParseError> {
        value.as_i64().ok_or_else(|| wrong_type(Self::KIND))
    }
}

/// Implements `CommandArg` for an integer type with the limits of the type.
macro_rules! integer_arg {
    ($($kind:ty),*) => {$(
        impl CommandArg for $kind {
            const KIND: CommandOptionType = CommandOptionType::Integer;

            fn configure(mut option: CommandOption) -> CommandOption {
                option.min_value = Some(NumberLimit::Integer(
                    i64::try_from(<$kind>::MIN).unwrap_or(-MAX_SAFE_INTEGER).max(-MAX_SAFE_INTEGER),
                ));
                option.max_value = Some(NumberLimit::Integer(
                    i64::try_from(<$kind>::MAX).unwrap_or(MAX_SAFE_INTEGER).min(MAX_SAFE_INTEGER),
                ));
                option
            }

            fn parse(value: &CommandOptionValue, _: Option<&Resolved>) -> Result<Self, ParseError> {
                let integer = value.as_i64().ok_or_else(|| wrong_type(Self::KIND))?;
                <$kind>::try_from(integer).map_err(|_| ParseError::new(ParseErrorKind::OutOfRange))
            }
        }
    )*};
}

integer_arg!(i8, i16, i32, u8, u16, u32, u64);

/// Implements `CommandArg` for an ID type.
macro_rules! id_arg {
    ($($id:ty => $kind:ident),*) => {$(
        impl CommandArg for $id {
            const KIND: CommandOptionType = CommandOptionType::$kind;

            fn parse(value: &CommandOptionValue, _: Option<&Resolved>) -> Result<Self, ParseError> {
                value.as_id().ok_or_else(|| wrong_type(Self::KIND))
            }
        }
    )*};
}

id_arg!(UserId => User, ChannelId => Channel, RoleId => Role, GenericId => Mentionable);

/// A user option with the member data, when the command ran in a guild.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResolvedUser {
    /// The user.
    pub user: User,
    /// The member. It is `None` outside of guilds and when the user is not a member.
    pub member: Option<PartialMember>,
}

impl ResolvedUser {
    pub(crate) fn resolve(
        user_id: UserId,
        resolved: Option<&Resolved>,
    ) -> Result<Self, ParseError> {
        let resolved = resolved.ok_or(ParseError::new(ParseErrorKind::Unresolved))?;
        let user = resolved
            .users
            .get(&user_id)
            .cloned()
            .ok_or(ParseError::new(ParseErrorKind::Unresolved))?;
        Ok(Self {
            member: resolved.members.get(&user_id).cloned(),
            user,
        })
    }
}

impl CommandArg for ResolvedUser {
    const KIND: CommandOptionType = CommandOptionType::User;

    fn parse(value: &CommandOptionValue, resolved: Option<&Resolved>) -> Result<Self, ParseError> {
        let user_id = value.as_id().ok_or_else(|| wrong_type(Self::KIND))?;
        Self::resolve(user_id, resolved)
    }
}

impl CommandArg for User {
    const KIND: CommandOptionType = CommandOptionType::User;

    fn parse(value: &CommandOptionValue, resolved: Option<&Resolved>) -> Result<Self, ParseError> {
        ResolvedUser::parse(value, resolved).map(|resolved| resolved.user)
    }
}

impl CommandArg for Role {
    const KIND: CommandOptionType = CommandOptionType::Role;

    fn parse(value: &CommandOptionValue, resolved: Option<&Resolved>) -> Result<Self, ParseError> {
        let role_id: RoleId = value.as_id().ok_or_else(|| wrong_type(Self::KIND))?;
        resolved
            .and_then(|resolved| resolved.roles.get(&role_id))
            .cloned()
            .ok_or(ParseError::new(ParseErrorKind::Unresolved))
    }
}

/// A resolved channel is partial: it has the ID, name, type, permissions, and thread data.
impl CommandArg for Channel {
    const KIND: CommandOptionType = CommandOptionType::Channel;

    fn parse(value: &CommandOptionValue, resolved: Option<&Resolved>) -> Result<Self, ParseError> {
        let channel_id: ChannelId = value.as_id().ok_or_else(|| wrong_type(Self::KIND))?;
        resolved
            .and_then(|resolved| resolved.channels.get(&channel_id))
            .cloned()
            .ok_or(ParseError::new(ParseErrorKind::Unresolved))
    }
}

impl CommandArg for Attachment {
    const KIND: CommandOptionType = CommandOptionType::Attachment;

    fn parse(value: &CommandOptionValue, resolved: Option<&Resolved>) -> Result<Self, ParseError> {
        let attachment_id = value.as_id().ok_or_else(|| wrong_type(Self::KIND))?;
        resolved
            .and_then(|resolved| resolved.attachments.get(&attachment_id))
            .cloned()
            .ok_or(ParseError::new(ParseErrorKind::Unresolved))
    }
}

/// A mentionable option: a user or a role.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Mentionable {
    /// A user.
    User(Box<ResolvedUser>),
    /// A role.
    Role(Role),
}

impl CommandArg for Mentionable {
    const KIND: CommandOptionType = CommandOptionType::Mentionable;

    fn parse(value: &CommandOptionValue, resolved: Option<&Resolved>) -> Result<Self, ParseError> {
        let id: GenericId = value.as_id().ok_or_else(|| wrong_type(Self::KIND))?;
        if let Some(role) = resolved.and_then(|resolved| resolved.roles.get(&id.cast())) {
            return Ok(Self::Role(role.clone()));
        }
        ResolvedUser::resolve(id.cast(), resolved).map(|user| Self::User(Box::new(user)))
    }
}
