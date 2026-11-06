//! Pagination: read long lists page by page.
//!
//! ```no_run
//! # async fn run(client: cordial_http::Client) -> Result<(), cordial_http::Error> {
//! use cordial_model::id::ChannelId;
//!
//! let mut history = client.message_history(ChannelId::new_const(1234));
//! while let Some(page) = history.next_page().await? {
//!     for message in page {
//!         println!("{}", message.content);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use std::fmt;

use cordial_model::emoji::ReactionType;
use cordial_model::guild::{Ban, CurrentUserGuild, Member};
use cordial_model::id::{ApplicationId, AuditLogEntryId, ChannelId, GuildId, MessageId, UserId};
use cordial_model::message::Message;
use cordial_model::monetization::Entitlement;
use cordial_model::user::User;

use crate::client::Client;
use crate::error::Error;
use crate::transport::BoxFuture;

type Fetch<T, C> = Box<dyn FnMut(Option<C>) -> BoxFuture<'static, Result<Vec<T>, Error>> + Send>;
type CursorOf<T, C> = Box<dyn Fn(&T) -> Option<C> + Send>;

/// Reads a list page by page with a cursor.
///
/// The list ends when a page is smaller than the page size, or when an item has no cursor.
pub struct Pages<T, C> {
    fetch: Fetch<T, C>,
    cursor_of: CursorOf<T, C>,
    cursor: Option<C>,
    page_size: usize,
    done: bool,
}

impl<T, C> fmt::Debug for Pages<T, C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Pages")
            .field("page_size", &self.page_size)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl<T: Send + 'static, C: Clone + Send + 'static> Pages<T, C> {
    /// Makes a pager.
    ///
    /// `fetch` gets one page after the cursor (`None` for the first page). `cursor_of` gives
    /// the cursor of an item. The cursor of the last item of a page is the next cursor.
    pub fn new(
        page_size: usize,
        fetch: impl FnMut(Option<C>) -> BoxFuture<'static, Result<Vec<T>, Error>> + Send + 'static,
        cursor_of: impl Fn(&T) -> Option<C> + Send + 'static,
    ) -> Self {
        Self {
            fetch: Box::new(fetch),
            cursor_of: Box::new(cursor_of),
            cursor: None,
            page_size: page_size.max(1),
            done: false,
        }
    }

    /// Gets the next page. Gives `None` after the last page.
    ///
    /// # Errors
    ///
    /// Gives the error of the request. A later call tries the same page again.
    pub async fn next_page(&mut self) -> Result<Option<Vec<T>>, Error> {
        if self.done {
            return Ok(None);
        }

        let page = (self.fetch)(self.cursor.clone()).await?;
        if page.len() < self.page_size {
            self.done = true;
        }
        match page.last().and_then(|item| (self.cursor_of)(item)) {
            Some(cursor) => self.cursor = Some(cursor),
            None => self.done = true,
        }

        if page.is_empty() {
            return Ok(None);
        }
        Ok(Some(page))
    }

    /// Gets all items, up to `limit` items when it is set.
    ///
    /// # Errors
    ///
    /// Gives the error of the first request that fails.
    pub async fn collect(mut self, limit: Option<usize>) -> Result<Vec<T>, Error> {
        let mut items = Vec::new();
        while let Some(page) = self.next_page().await? {
            items.extend(page);
            if let Some(limit) = limit
                && items.len() >= limit
            {
                items.truncate(limit);
                break;
            }
        }
        Ok(items)
    }
}

impl Client {
    /// Reads the messages of a channel from newest to oldest, 100 for each page.
    pub fn message_history(&self, channel_id: ChannelId) -> Pages<Message, MessageId> {
        let client = self.clone();
        Pages::new(
            100,
            move |before| {
                let mut request = client.messages(channel_id).limit(100);
                if let Some(before) = before {
                    request = request.before(before);
                }
                Box::pin(request.into_future_boxed())
            },
            |message: &Message| Some(message.id),
        )
    }

    /// Reads all members of a guild, 1000 for each page. Needs the `GUILD_MEMBERS` intent.
    pub fn all_guild_members(&self, guild_id: GuildId) -> Pages<Member, UserId> {
        let client = self.clone();
        Pages::new(
            1000,
            move |after| {
                let mut request = client.guild_members(guild_id).limit(1000);
                if let Some(after) = after {
                    request = request.after(after);
                }
                Box::pin(request.into_future_boxed())
            },
            |member: &Member| member.user_id(),
        )
    }

    /// Reads all bans of a guild, 1000 for each page.
    pub fn all_guild_bans(&self, guild_id: GuildId) -> Pages<Ban, UserId> {
        let client = self.clone();
        Pages::new(
            1000,
            move |after| {
                let mut request = client.guild_bans(guild_id).limit(1000);
                if let Some(after) = after {
                    request = request.after(after);
                }
                Box::pin(request.into_future_boxed())
            },
            |ban: &Ban| Some(ban.user.id),
        )
    }

    /// Reads all users that reacted with an emoji, 100 for each page.
    pub fn all_reactions(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        emoji: impl Into<ReactionType>,
    ) -> Pages<User, UserId> {
        let client = self.clone();
        let emoji = emoji.into();
        Pages::new(
            100,
            move |after| {
                let mut request = client
                    .reactions(channel_id, message_id, emoji.clone())
                    .limit(100);
                if let Some(after) = after {
                    request = request.after(after);
                }
                Box::pin(request.into_future_boxed())
            },
            |user: &User| Some(user.id),
        )
    }

    /// Reads all guilds of the current user, 200 for each page.
    pub fn all_current_user_guilds(&self) -> Pages<CurrentUserGuild, GuildId> {
        let client = self.clone();
        Pages::new(
            200,
            move |after| {
                let mut request = client.current_user_guilds().limit(200);
                if let Some(after) = after {
                    request = request.after(after);
                }
                Box::pin(request.into_future_boxed())
            },
            |guild: &CurrentUserGuild| Some(guild.id),
        )
    }

    /// Reads all entitlements of an application, 100 for each page.
    pub fn all_entitlements(
        &self,
        application_id: ApplicationId,
    ) -> Pages<Entitlement, cordial_model::id::EntitlementId> {
        let client = self.clone();
        Pages::new(
            100,
            move |after| {
                let mut request = client.entitlements(application_id).limit(100);
                if let Some(after) = after {
                    request = request.after(after);
                }
                Box::pin(request.into_future_boxed())
            },
            |entitlement: &Entitlement| Some(entitlement.id),
        )
    }

    /// Reads the audit log entries of a guild from newest to oldest, 100 for each page.
    ///
    /// The objects that the entries refer to (users, webhooks, and others) are not kept. Use
    /// `guild_audit_log` for them.
    pub fn audit_log_history(
        &self,
        guild_id: GuildId,
    ) -> Pages<cordial_model::audit_log::AuditLogEntry, AuditLogEntryId> {
        let client = self.clone();
        Pages::new(
            100,
            move |before| {
                let mut request = client.guild_audit_log(guild_id).limit(100);
                if let Some(before) = before {
                    request = request.before(before);
                }
                Box::pin(async move { request.await.map(|log| log.audit_log_entries) })
            },
            |entry: &cordial_model::audit_log::AuditLogEntry| Some(entry.id),
        )
    }
}

/// Turns a request into a boxed future of a list.
trait IntoFutureBoxed<T> {
    fn into_future_boxed(self) -> BoxFuture<'static, Result<Vec<T>, Error>>;
}

impl<R, T> IntoFutureBoxed<T> for R
where
    R: std::future::IntoFuture<Output = Result<Vec<T>, Error>> + Send + 'static,
    R::IntoFuture: Send + 'static,
{
    fn into_future_boxed(self) -> BoxFuture<'static, Result<Vec<T>, Error>> {
        Box::pin(self.into_future())
    }
}
