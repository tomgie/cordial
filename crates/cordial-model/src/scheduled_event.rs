//! Guild scheduled events.

use serde::{Deserialize, Serialize};

use crate::guild::Member;
use crate::id::{ChannelId, GenericId, GuildId, ScheduledEventId, UserId};
use crate::user::User;
use crate::{ImageHash, Timestamp};

/// A scheduled event in a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ScheduledEvent {
    /// The event ID.
    pub id: ScheduledEventId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The channel of the event. `None` for external events.
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    /// The user that made the event. `None` for events made before 2021-10-25.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_id: Option<UserId>,
    /// The name, 1 to 100 characters.
    pub name: String,
    /// The description, 1 to 1000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When the event starts.
    pub scheduled_start_time: Timestamp,
    /// When the event ends. Required for external events.
    #[serde(default)]
    pub scheduled_end_time: Option<Timestamp>,
    /// The privacy level.
    #[serde(default)]
    pub privacy_level: ScheduledEventPrivacyLevel,
    /// The status.
    #[serde(default)]
    pub status: ScheduledEventStatus,
    /// The entity type.
    #[serde(default)]
    pub entity_type: ScheduledEventEntityType,
    /// The ID of an entity of the event.
    #[serde(default)]
    pub entity_id: Option<GenericId>,
    /// More data for the event. Required for external events.
    #[serde(default)]
    pub entity_metadata: Option<ScheduledEventEntityMetadata>,
    /// The user that made the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<User>,
    /// The number of users that subscribed to the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_count: Option<u64>,
    /// The cover image hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageHash>,
    /// How often the event recurs.
    #[serde(default)]
    pub recurrence_rule: Option<RecurrenceRule>,
}

crate::discord_enum! {
    /// The privacy level of a scheduled event.
    pub enum ScheduledEventPrivacyLevel: u8 {
        /// Only guild members can access the event.
        GuildOnly = 2,
    }
}

crate::discord_enum! {
    /// The entity type of a scheduled event.
    pub enum ScheduledEventEntityType: u8 {
        /// The event is in a stage channel.
        StageInstance = 1,
        /// The event is in a voice channel.
        Voice = 2,
        /// The event is at an external location.
        External = 3,
    }
}

crate::discord_enum! {
    /// The status of a scheduled event.
    ///
    /// Valid changes: scheduled to active, active to completed, scheduled to canceled.
    pub enum ScheduledEventStatus: u8 {
        /// The event is scheduled.
        Scheduled = 1,
        /// The event is active.
        Active = 2,
        /// The event is completed. The status cannot change after this.
        Completed = 3,
        /// The event is canceled. The status cannot change after this.
        Canceled = 4,
    }
}

/// More data for a scheduled event.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ScheduledEventEntityMetadata {
    /// The location, 1 to 100 characters. Required for external events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

impl ScheduledEventEntityMetadata {
    /// Makes metadata with a location.
    #[must_use]
    pub fn location(location: impl Into<String>) -> Self {
        Self {
            location: Some(location.into()),
        }
    }
}

/// A user that subscribed to a scheduled event.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ScheduledEventUser {
    /// The event ID.
    pub guild_scheduled_event_id: ScheduledEventId,
    /// The user.
    pub user: User,
    /// The member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
}

/// How often a scheduled event recurs. A subset of iCalendar (RFC 5545) rules.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RecurrenceRule {
    /// The start of the recurrence.
    pub start: Timestamp,
    /// The end of the recurrence. Applications cannot set it.
    #[serde(default)]
    pub end: Option<Timestamp>,
    /// How often the event occurs.
    pub frequency: RecurrenceFrequency,
    /// The spacing between events, in units of `frequency`.
    #[serde(default = "one")]
    pub interval: u32,
    /// The days of the week. Only for daily and weekly events.
    #[serde(default)]
    pub by_weekday: Option<Vec<Weekday>>,
    /// Specific days in specific weeks. Only for monthly events.
    #[serde(default)]
    pub by_n_weekday: Option<Vec<NWeekday>>,
    /// The months. Only for yearly events, together with `by_month_day`.
    #[serde(default)]
    pub by_month: Option<Vec<Month>>,
    /// The days of the month. Only for yearly events, together with `by_month`.
    #[serde(default)]
    pub by_month_day: Option<Vec<u8>>,
    /// The days of the year. Applications cannot set it.
    #[serde(default)]
    pub by_year_day: Option<Vec<u16>>,
    /// The number of times that the event recurs. Applications cannot set it.
    #[serde(default)]
    pub count: Option<u32>,
}

fn one() -> u32 {
    1
}

impl RecurrenceRule {
    /// Makes a rule that starts at `start` and repeats at `frequency`, with interval 1.
    ///
    /// Discord limits the combinations: see the setters.
    #[must_use]
    pub fn new(start: Timestamp, frequency: RecurrenceFrequency) -> Self {
        Self {
            start,
            end: None,
            frequency,
            interval: 1,
            by_weekday: None,
            by_n_weekday: None,
            by_month: None,
            by_month_day: None,
            by_year_day: None,
            count: None,
        }
    }

    /// Sets the interval. Only weekly events can use 2 ("every other week").
    #[must_use]
    pub fn interval(mut self, interval: u32) -> Self {
        self.interval = interval;
        self
    }

    /// Sets the days of the week. Daily events need a fixed set; weekly events need one day.
    #[must_use]
    pub fn by_weekday(mut self, days: Vec<Weekday>) -> Self {
        self.by_weekday = Some(days);
        self
    }

    /// Sets one day in one week of the month. Only for monthly events.
    #[must_use]
    pub fn by_n_weekday(mut self, day: NWeekday) -> Self {
        self.by_n_weekday = Some(vec![day]);
        self
    }

    /// Sets the date of a yearly event.
    #[must_use]
    pub fn by_date(mut self, month: Month, day: u8) -> Self {
        self.by_month = Some(vec![month]);
        self.by_month_day = Some(vec![day]);
        self
    }
}

crate::discord_enum! {
    /// How often a scheduled event occurs.
    pub enum RecurrenceFrequency: u8 {
        /// Each year.
        Yearly = 0,
        /// Each month.
        Monthly = 1,
        /// Each week.
        Weekly = 2,
        /// Each day.
        Daily = 3,
    }
}

crate::discord_enum! {
    /// A day of the week.
    pub enum Weekday: u8 {
        /// Monday.
        Monday = 0,
        /// Tuesday.
        Tuesday = 1,
        /// Wednesday.
        Wednesday = 2,
        /// Thursday.
        Thursday = 3,
        /// Friday.
        Friday = 4,
        /// Saturday.
        Saturday = 5,
        /// Sunday.
        Sunday = 6,
    }
}

crate::discord_enum! {
    /// A month.
    pub enum Month: u8 {
        /// January.
        January = 1,
        /// February.
        February = 2,
        /// March.
        March = 3,
        /// April.
        April = 4,
        /// May.
        May = 5,
        /// June.
        June = 6,
        /// July.
        July = 7,
        /// August.
        August = 8,
        /// September.
        September = 9,
        /// October.
        October = 10,
        /// November.
        November = 11,
        /// December.
        December = 12,
    }
}

/// A specific day in a specific week of the month.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct NWeekday {
    /// The week of the month, 1 to 5.
    pub n: u8,
    /// The day of the week.
    pub day: Weekday,
}

impl NWeekday {
    /// Makes a day in a week of the month.
    #[must_use]
    pub const fn new(n: u8, day: Weekday) -> Self {
        Self { n, day }
    }
}
