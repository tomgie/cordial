//! JSON error codes of the Discord API.
//!
//! Generated from the table "JSON Error Codes" in `developers/topics/opcodes-and-status-codes.mdx`
//! (discord-api-docs commit `da52492`). Do not edit by hand.

cordial_model::discord_enum! {
    /// A JSON error code in an error response of the Discord API.
    pub enum JsonErrorCode: u32 {
        /// General error (such as a malformed request body, amongst other things).
        GeneralError = 0,
        /// Unknown account.
        UnknownAccount = 10001,
        /// Unknown application.
        UnknownApplication = 10002,
        /// Unknown channel.
        UnknownChannel = 10003,
        /// Unknown guild.
        UnknownGuild = 10004,
        /// Unknown integration.
        UnknownIntegration = 10005,
        /// Unknown invite.
        UnknownInvite = 10006,
        /// Unknown member.
        UnknownMember = 10007,
        /// Unknown message.
        UnknownMessage = 10008,
        /// Unknown permission overwrite.
        UnknownPermissionOverwrite = 10009,
        /// Unknown provider.
        UnknownProvider = 10010,
        /// Unknown role.
        UnknownRole = 10011,
        /// Unknown token.
        UnknownToken = 10012,
        /// Unknown user.
        UnknownUser = 10013,
        /// Unknown emoji.
        UnknownEmoji = 10014,
        /// Unknown webhook.
        UnknownWebhook = 10015,
        /// Unknown webhook service.
        UnknownWebhookService = 10016,
        /// Unknown session.
        UnknownSession = 10020,
        /// Unknown Asset.
        UnknownAsset = 10021,
        /// Unknown ban.
        UnknownBan = 10026,
        /// Unknown SKU.
        UnknownSku = 10027,
        /// Unknown Store Listing.
        UnknownStoreListing = 10028,
        /// Unknown entitlement.
        UnknownEntitlement = 10029,
        /// Unknown build.
        UnknownBuild = 10030,
        /// Unknown lobby.
        UnknownLobby = 10031,
        /// Unknown branch.
        UnknownBranch = 10032,
        /// Unknown store directory layout.
        UnknownStoreDirectoryLayout = 10033,
        /// Unknown redistributable.
        UnknownRedistributable = 10036,
        /// Unknown gift code.
        UnknownGiftCode = 10038,
        /// Unknown stream.
        UnknownStream = 10049,
        /// Unknown premium server subscribe cooldown.
        UnknownPremiumServerSubscribeCooldown = 10050,
        /// Unknown guild template.
        UnknownGuildTemplate = 10057,
        /// Unknown discoverable server category.
        UnknownDiscoverableServerCategory = 10059,
        /// Unknown sticker.
        UnknownSticker = 10060,
        /// Unknown sticker pack.
        UnknownStickerPack = 10061,
        /// Unknown interaction.
        UnknownInteraction = 10062,
        /// Unknown application command.
        UnknownApplicationCommand = 10063,
        /// Unknown voice state.
        UnknownVoiceState = 10065,
        /// Unknown application command permissions.
        UnknownApplicationCommandPermissions = 10066,
        /// Unknown Stage Instance.
        UnknownStageInstance = 10067,
        /// Unknown Guild Member Verification Form.
        UnknownGuildMemberVerificationForm = 10068,
        /// Unknown Guild Welcome Screen.
        UnknownGuildWelcomeScreen = 10069,
        /// Unknown Guild Scheduled Event.
        UnknownGuildScheduledEvent = 10070,
        /// Unknown Guild Scheduled Event User.
        UnknownGuildScheduledEventUser = 10071,
        /// Unknown Tag.
        UnknownTag = 10087,
        /// Unknown sound.
        UnknownSound = 10097,
        /// Unknown invite target users job (invite exists but has no target users).
        UnknownInviteTargetUsersJob = 10124,
        /// Unknown invite target users (invite exists but has no target users).
        UnknownInviteTargetUsers = 10129,
        /// Bots cannot use this endpoint.
        BotsCannotUseEndpoint = 20001,
        /// Only bots can use this endpoint.
        OnlyBotsCanUseEndpoint = 20002,
        /// Explicit content cannot be sent to the desired recipient(s).
        ExplicitContentCannotSentDesiredRecipient = 20009,
        /// You are not authorized to perform this action on this application.
        NotAuthorizedPerformActionApplication = 20012,
        /// This action requires a premium subscription.
        ActionRequiresPremiumSubscription = 20015,
        /// This action cannot be performed due to slowmode rate limit.
        SlowmodeRateLimit = 20016,
        /// Only the owner of this account can perform this action.
        OnlyOwnerAccountCanPerformAction = 20018,
        /// This message cannot be edited due to announcement rate limits.
        AnnouncementEditRateLimit = 20022,
        /// Under minimum age.
        UnderMinimumAge = 20024,
        /// The write action you are performing on the channel has hit the write rate limit.
        ChannelWriteRateLimit = 20028,
        /// The write action you are performing on the server has hit the write rate limit.
        ServerWriteRateLimit = 20029,
        /// Your Stage topic, server name, server description, or channel names contain words that are not allowed.
        StageTopicServerNameServerDescriptionChannel = 20031,
        /// Guild premium subscription level too low.
        GuildPremiumSubscriptionLevelTooLow = 20035,
        /// Maximum number of guilds reached (100).
        MaximumNumberGuildsReached = 30001,
        /// Maximum number of friends reached (1000).
        MaximumNumberFriendsReached = 30002,
        /// Maximum number of pins reached for the channel (250).
        MaximumPinsReached = 30003,
        /// Maximum number of recipients reached (10).
        MaximumNumberRecipientsReached = 30004,
        /// Maximum number of guild roles reached (250).
        MaximumNumberGuildRolesReached = 30005,
        /// Maximum number of webhooks reached (15).
        MaximumNumberWebhooksReached = 30007,
        /// Maximum number of emojis reached.
        MaximumNumberEmojisReached = 30008,
        /// Maximum number of reactions reached (20).
        MaximumNumberReactionsReached = 30010,
        /// Maximum number of group DMs reached (10).
        MaximumNumberGroupDmsReached = 30011,
        /// Maximum number of guild channels reached (500).
        MaximumNumberGuildChannelsReached = 30013,
        /// Maximum number of attachments in a message reached (10).
        MaximumNumberAttachmentsMessageReached = 30015,
        /// Maximum number of invites reached (1000).
        MaximumNumberInvitesReached = 30016,
        /// Maximum number of animated emojis reached.
        MaximumNumberAnimatedEmojisReached = 30018,
        /// Maximum number of server members reached.
        MaximumNumberServerMembersReached = 30019,
        /// Maximum number of server categories has been reached (5).
        MaximumNumberServerCategoriesHasBeenReached = 30030,
        /// Guild already has a template.
        GuildAlreadyHasTemplate = 30031,
        /// Maximum number of application commands reached.
        MaximumNumberApplicationCommandsReached = 30032,
        /// Maximum number of thread participants has been reached (1000).
        MaximumNumberThreadParticipantsHasBeenReached = 30033,
        /// Maximum number of daily application command creates has been reached (200).
        DailyCommandCreateLimit = 30034,
        /// Maximum number of bans for non-guild members have been exceeded.
        MaximumNumberBansNonGuildMembersHave = 30035,
        /// Maximum number of bans fetches has been reached.
        MaximumNumberBansFetchesHasBeenReached = 30037,
        /// Maximum number of uncompleted guild scheduled events reached (100).
        MaximumNumberUncompletedGuildScheduledEventsReached = 30038,
        /// Maximum number of stickers reached.
        MaximumNumberStickersReached = 30039,
        /// Maximum number of prune requests has been reached. Try again later.
        MaximumNumberPruneRequestsHasBeenReached = 30040,
        /// Maximum number of guild widget settings updates has been reached. Try again later.
        MaximumNumberGuildWidgetSettingsUpdatesHas = 30042,
        /// Maximum number of soundboard sounds reached.
        MaximumNumberSoundboardSoundsReached = 30045,
        /// Maximum number of edits to messages older than 1 hour reached. Try again later.
        OldMessageEditLimit = 30046,
        /// Maximum number of pinned threads in a forum channel has been reached.
        MaximumNumberPinnedThreadsForumChannelHas = 30047,
        /// Maximum number of tags in a forum channel has been reached.
        MaximumNumberTagsForumChannelHasBeen = 30048,
        /// Bitrate is too high for channel of this type.
        BitrateTooHighChannelType = 30052,
        /// Maximum number of premium emojis reached (25).
        MaximumNumberPremiumEmojisReached = 30056,
        /// Maximum number of webhooks per guild reached (1000).
        MaximumNumberWebhooksPerGuildReached = 30058,
        /// Maximum number of channel permission overwrites reached (1000).
        MaximumNumberChannelPermissionOverwritesReached = 30060,
        /// The channels for this guild are too large.
        ChannelsGuildTooLarge = 30061,
        /// Target users limit for this guild reached.
        TargetUsersLimitGuildReached = 30084,
        /// Unauthorized. Provide a valid token and try again.
        Unauthorized = 40001,
        /// You need to verify your account in order to perform this action.
        AccountVerificationRequired = 40002,
        /// You are opening direct messages too fast.
        OpeningDirectMessagesTooFast = 40003,
        /// Send messages has been temporarily disabled.
        SendMessagesHasBeenTemporarilyDisabled = 40004,
        /// Request entity too large. Try sending something smaller in size.
        RequestEntityTooLarge = 40005,
        /// This feature has been temporarily disabled server-side.
        FeatureHasBeenTemporarilyDisabledServerSide = 40006,
        /// The user is banned from this guild.
        UserBannedFromGuild = 40007,
        /// Only one channel can have a parent_id modified at a time.
        OnlyOneChannelCanHaveParentId = 40009,
        /// Connection has been revoked.
        ConnectionHasBeenRevoked = 40012,
        /// Only consumable SKUs can be consumed.
        OnlyConsumableSkusCanConsumed = 40018,
        /// You can only delete sandbox entitlements.
        CanOnlyDeleteSandboxEntitlements = 40019,
        /// Target user is not connected to voice.
        TargetUserNotConnectedVoice = 40032,
        /// This message has already been crossposted.
        MessageHasAlreadyBeenCrossposted = 40033,
        /// An application command with that name already exists.
        ApplicationCommandNameAlreadyExists = 40041,
        /// Application interaction failed to send.
        ApplicationInteractionFailedSend = 40043,
        /// Cannot send a message in a forum channel.
        CannotSendMessageForumChannel = 40058,
        /// Interaction has already been acknowledged.
        InteractionAlreadyAcknowledged = 40060,
        /// Tag names must be unique.
        TagNamesMustUnique = 40061,
        /// Service resource is being rate limited.
        ServiceResourceRateLimited = 40062,
        /// There are no tags available that can be set by non-moderators.
        ThereNoTagsAvailableCanSetBy = 40066,
        /// A tag is required to create a forum post in this channel.
        TagRequiredCreateForumPostChannel = 40067,
        /// An entitlement has already been granted for this resource.
        EntitlementHasAlreadyBeenGrantedResource = 40074,
        /// This interaction has hit the maximum number of follow up messages.
        FollowUpLimitReached = 40094,
        /// Target users are already being processed for this invite.
        TargetUsersAlreadyBeingProcessedInvite = 40114,
        /// Target users file has not been processed.
        TargetUsersFileHasNotBeenProcessed = 40115,
        /// Cloudflare is blocking your request. This can often be resolved by setting a proper User Agent.
        CloudflareBlocked = 40333,
        /// Missing access.
        MissingAccess = 50001,
        /// Invalid account type.
        InvalidAccountType = 50002,
        /// Cannot execute action on a DM channel.
        CannotExecuteActionDmChannel = 50003,
        /// Guild widget disabled.
        GuildWidgetDisabled = 50004,
        /// Cannot edit a message authored by another user.
        CannotEditMessageAuthoredByAnotherUser = 50005,
        /// Cannot send an empty message.
        EmptyMessage = 50006,
        /// Cannot send messages to this user.
        CannotMessageUser = 50007,
        /// Cannot send messages in a non-text channel.
        CannotSendMessagesNonTextChannel = 50008,
        /// Channel verification level is too high for you to gain access.
        ChannelVerificationLevelTooHighGainAccess = 50009,
        /// OAuth2 application does not have a bot.
        Oauth2ApplicationDoesNotHaveBot = 50010,
        /// OAuth2 application limit reached.
        Oauth2ApplicationLimitReached = 50011,
        /// Invalid OAuth2 state.
        InvalidOauth2State = 50012,
        /// You lack permissions to perform that action.
        MissingPermissions = 50013,
        /// Invalid authentication token provided.
        InvalidToken = 50014,
        /// Note was too long.
        NoteWasTooLong = 50015,
        /// Provided too few or too many messages to delete. Must provide at least 2 and fewer than 100 messages to delete.
        BulkDeleteCountInvalid = 50016,
        /// Invalid MFA Level.
        InvalidMfaLevel = 50017,
        /// A message can only be pinned to the channel it was sent in.
        MessageCanOnlyPinnedChannelWasSent = 50019,
        /// Invite code was either invalid or taken.
        InviteCodeWasEitherInvalidTaken = 50020,
        /// Cannot execute action on a system message.
        CannotExecuteActionSystemMessage = 50021,
        /// Cannot execute action on this channel type.
        CannotExecuteActionChannelType = 50024,
        /// Invalid OAuth2 access token provided.
        InvalidOauth2AccessTokenProvided = 50025,
        /// Missing required OAuth2 scope.
        MissingRequiredOauth2Scope = 50026,
        /// Invalid webhook token provided.
        InvalidWebhookTokenProvided = 50027,
        /// Invalid role.
        InvalidRole = 50028,
        /// Invalid Recipient(s).
        InvalidRecipient = 50033,
        /// A message provided was too old to bulk delete.
        MessageTooOldToBulkDelete = 50034,
        /// Invalid form body (returned for both application/json and multipart/form-data bodies), or invalid Content-Type provided.
        InvalidFormBody = 50035,
        /// An invite was accepted to a guild the application's bot is not in.
        InviteWasAcceptedGuildApplicationSBot = 50036,
        /// Invalid Activity Action.
        InvalidActivityAction = 50039,
        /// Invalid API version provided.
        InvalidApiVersionProvided = 50041,
        /// File uploaded exceeds the maximum size.
        FileTooLarge = 50045,
        /// Invalid file uploaded.
        InvalidFileUploaded = 50046,
        /// Cannot self-redeem this gift.
        CannotSelfRedeemGift = 50054,
        /// Invalid Guild.
        InvalidGuild = 50055,
        /// Invalid SKU.
        InvalidSku = 50057,
        /// Invalid request origin.
        InvalidRequestOrigin = 50067,
        /// Invalid message type.
        InvalidMessageType = 50068,
        /// Payment source required to redeem gift.
        PaymentSourceRequiredRedeemGift = 50070,
        /// Cannot modify a system webhook.
        CannotModifySystemWebhook = 50073,
        /// Cannot delete a channel required for Community guilds.
        CannotDeleteChannelRequiredCommunityGuilds = 50074,
        /// Cannot edit stickers within a message.
        CannotEditStickersWithinMessage = 50080,
        /// Invalid sticker sent.
        InvalidStickerSent = 50081,
        /// Tried to perform an operation on an archived thread, such as editing a message or adding a user to the thread.
        TriedPerformOperationArchivedThreadEditingMessage = 50083,
        /// Invalid thread notification settings.
        InvalidThreadNotificationSettings = 50084,
        /// before value is earlier than the thread creation date.
        BeforeValueEarlierThanThreadCreationDate = 50085,
        /// Community server channels must be text channels.
        CommunityServerChannelsMustTextChannels = 50086,
        /// The entity type of the event is different from the entity you are trying to start the event for.
        EntityTypeEventDifferentFromEntityTrying = 50091,
        /// This server is not available in your location.
        ServerNotAvailableLocation = 50095,
        /// This server needs monetization enabled in order to perform this action.
        ServerNeedsMonetizationEnabledOrderPerformAction = 50097,
        /// This server needs more boosts to perform this action.
        ServerNeedsMoreBoostsPerformAction = 50101,
        /// The request body contains invalid JSON.
        InvalidJson = 50109,
        /// The provided file is invalid.
        ProvidedFileInvalid = 50110,
        /// The provided file type is invalid.
        ProvidedFileTypeInvalid = 50123,
        /// The provided file duration exceeds maximum of 5.2 seconds.
        ProvidedFileDurationExceedsMaximum52 = 50124,
        /// Owner cannot be pending member.
        OwnerCannotPendingMember = 50131,
        /// Ownership cannot be transferred to a bot user.
        OwnershipCannotTransferredBotUser = 50132,
        /// Failed to resize asset below the maximum size: 262144.
        FailedResizeAssetBelowMaximumSize262144 = 50138,
        /// Cannot mix subscription and non subscription roles for an emoji.
        CannotMixSubscriptionNonSubscriptionRolesEmoji = 50144,
        /// Cannot convert between premium emoji and normal emoji.
        CannotConvertBetweenPremiumEmojiNormalEmoji = 50145,
        /// Uploaded file not found.
        UploadedFileNotFound = 50146,
        /// The specified emoji is invalid.
        SpecifiedEmojiInvalid = 50151,
        /// Voice messages do not support additional content.
        VoiceMessagesDoNotSupportAdditionalContent = 50159,
        /// Voice messages must have a single audio attachment.
        VoiceMessagesMustHaveSingleAudioAttachment = 50160,
        /// Voice messages must have supporting metadata.
        VoiceMessagesMustHaveSupportingMetadata = 50161,
        /// Voice messages cannot be edited.
        VoiceMessagesCannotEdited = 50162,
        /// Cannot delete guild subscription integration.
        CannotDeleteGuildSubscriptionIntegration = 50163,
        /// Cannot send voice effect when user is server muted, deafened, or suppressed.
        CannotSendVoiceEffectWhenUserServer = 50167,
        /// You cannot send voice messages in this channel.
        CannotSendVoiceMessagesChannel = 50173,
        /// The user account must first be verified.
        UserAccountMustFirstVerified = 50178,
        /// The provided file does not have a valid duration.
        ProvidedFileDoesNotHaveValidDuration = 50192,
        /// Cannot send messages to this user due to having no mutual guilds.
        NoMutualGuilds = 50278,
        /// You do not have permission to send this sticker.
        DoNotHavePermissionSendSticker = 50600,
        /// Two factor is required for this operation.
        TwoFactorRequired = 60003,
        /// No users with DiscordTag exist.
        NoUsersDiscordtagExist = 80004,
        /// Reaction was blocked.
        ReactionWasBlocked = 90001,
        /// User cannot use burst reactions.
        UserCannotUseBurstReactions = 90002,
        /// Index not yet available. Try again later.
        IndexNotReady = 110000,
        /// Application not yet available. Try again later.
        ApplicationNotReady = 110001,
        /// API resource is currently overloaded. Try again a little later.
        ResourceOverloaded = 130000,
        /// The Stage is already open.
        StageAlreadyOpen = 150006,
        /// Cannot reply without permission to read message history.
        ReplyNeedsReadMessageHistory = 160002,
        /// A thread has already been created for this message.
        ThreadHasAlreadyBeenCreatedMessage = 160004,
        /// Thread is locked.
        ThreadLocked = 160005,
        /// Maximum number of active threads reached.
        MaximumNumberActiveThreadsReached = 160006,
        /// Maximum number of active announcement threads reached.
        MaximumNumberActiveAnnouncementThreadsReached = 160007,
        /// Cannot reference a message without permission to read message history.
        ReferenceNeedsReadMessageHistory = 160009,
        /// NSFW channel message reference not allowed.
        NsfwChannelMessageReferenceNotAllowed = 160010,
        /// You cannot forward a message whose content you cannot read.
        CannotForwardUnreadableMessage = 160014,
        /// Invalid JSON for uploaded Lottie file.
        InvalidJsonUploadedLottieFile = 170001,
        /// Uploaded Lotties cannot contain rasterized images such as PNG or JPEG.
        UploadedLottiesCannotContainRasterizedImagesPng = 170002,
        /// Sticker maximum framerate exceeded.
        StickerMaximumFramerateExceeded = 170003,
        /// Sticker frame count exceeds maximum of 1000 frames.
        StickerFrameCountExceedsMaximum1000Frames = 170004,
        /// Lottie animation maximum dimensions exceeded.
        LottieAnimationMaximumDimensionsExceeded = 170005,
        /// Sticker frame rate is either too small or too large.
        StickerFrameRateEitherTooSmallToo = 170006,
        /// Sticker animation duration exceeds maximum of 5 seconds.
        StickerAnimationDurationExceedsMaximum5Seconds = 170007,
        /// Cannot update a finished event.
        CannotUpdateFinishedEvent = 180000,
        /// Failed to create stage needed for stage event.
        FailedCreateStageNeededStageEvent = 180002,
        /// Message was blocked by automatic moderation.
        BlockedByAutoModeration = 200000,
        /// Title was blocked by automatic moderation.
        TitleBlockedByAutoModeration = 200001,
        /// Webhooks posted to forum channels must have a thread_name or thread_id.
        WebhooksPostedForumChannelsMustHaveThread = 220001,
        /// Webhooks posted to forum channels cannot have both a thread_name and thread_id.
        WebhooksPostedForumChannelsCannotHaveBoth = 220002,
        /// Webhooks can only create threads in forum channels.
        WebhooksCanOnlyCreateThreadsForumChannels = 220003,
        /// Webhook services cannot be used in forum channels.
        WebhookServicesCannotUsedForumChannels = 220004,
        /// Message blocked by harmful links filter.
        MessageBlockedByHarmfulLinksFilter = 240000,
        /// Access to joining new servers has been limited for the user.
        AccessJoiningNewServersHasBeenLimited = 340015,
        /// Cannot enable onboarding, requirements are not met.
        CannotEnableOnboardingRequirementsNotMet = 350000,
        /// Cannot update onboarding while below requirements.
        CannotUpdateOnboardingWhileBelowRequirements = 350001,
        /// Access to file uploads has been limited for this guild.
        AccessFileUploadsHasBeenLimitedGuild = 400001,
        /// Failed to ban users.
        FailedBanUsers = 500000,
        /// Poll voting blocked.
        PollVotingBlocked = 520000,
        /// Poll expired.
        PollExpired = 520001,
        /// Invalid channel type for poll creation.
        InvalidChannelTypePollCreation = 520002,
        /// Cannot edit a poll message.
        CannotEditPollMessage = 520003,
        /// Cannot use an emoji included with the poll.
        CannotUseEmojiIncludedPoll = 520004,
        /// Cannot expire a non-poll message.
        CannotExpireNonPollMessage = 520006,
        /// Your Discord application has not been granted the permission to use provisional accounts.
        DiscordApplicationHasNotBeenGrantedPermission = 530000,
        /// The ID token JWT you have provided is expired. You will need to get another one issued from the identity provider.
        IdTokenJwtHaveProvidedExpiredWill = 530001,
        /// The issuer in the ID token JWT you have provided does not match what you have configured.
        IssuerIdTokenJwtHaveProvidedDoes = 530002,
        /// The audience in the ID token JWT you have provided does not match the audience you specified in your OIDC configuration. Either update your configuration or pass in an ID token that was issued to your application.
        AudienceIdTokenJwtHaveProvidedDoes = 530003,
        /// The ID token you provided was issued too long ago. Discord will not accept ID tokens issued beyond a week ago. You will need to get a new ID token issued from the identity provider.
        IdTokenProvidedWasIssuedTooLong = 530004,
        /// Discord failed to generate a unique username within the allotted time. This is not a terminal error, and should resolve itself upon a retry.
        DiscordFailedGenerateUniqueUsernameWithinAllotted = 530006,
        /// Your client secret is invalid. Double check what you are sending or regenerate your client secret.
        ClientSecretInvalidDoubleCheckWhatSending = 530007,
    }
}
