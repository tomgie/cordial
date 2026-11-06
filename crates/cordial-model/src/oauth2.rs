//! OAuth2 scopes and related types.

crate::discord_string_enum! {
    /// An OAuth2 scope.
    pub enum Scope {
        /// Read the activity data of the user.
        ActivitiesRead = "activities.read",
        /// Update the activity of the user.
        ActivitiesWrite = "activities.write",
        /// Read the build data of the applications of the user.
        ApplicationsBuildsRead = "applications.builds.read",
        /// Upload builds for the applications of the user.
        ApplicationsBuildsUpload = "applications.builds.upload",
        /// Use commands in a guild.
        ApplicationsCommands = "applications.commands",
        /// Update the commands of the application. Client credentials grant only.
        ApplicationsCommandsUpdate = "applications.commands.update",
        /// Update the command permissions in the guilds of the user.
        ApplicationsCommandsPermissionsUpdate = "applications.commands.permissions.update",
        /// Read the entitlements of the user for the application.
        ApplicationsEntitlements = "applications.entitlements",
        /// Read and update the store data of the applications of the user.
        ApplicationsStoreUpdate = "applications.store.update",
        /// Add a bot to a guild.
        Bot = "bot",
        /// Read the connections of the user.
        Connections = "connections",
        /// Read the DM channels of the user.
        DmChannelsRead = "dm_channels.read",
        /// Read the email of the user.
        Email = "email",
        /// Join users to a group DM.
        GdmJoin = "gdm.join",
        /// Read the guilds of the user.
        Guilds = "guilds",
        /// Join users to a guild.
        GuildsJoin = "guilds.join",
        /// Read the member data of the user in guilds.
        GuildsMembersRead = "guilds.members.read",
        /// Read the user (without email).
        Identify = "identify",
        /// Read the Nitro type of the user. For approved partners only.
        IdentifyPremium = "identify.premium",
        /// Read messages (local RPC only).
        MessagesRead = "messages.read",
        /// Read the relationships of the user.
        RelationshipsRead = "relationships.read",
        /// Update the role connection of the user.
        RoleConnectionsWrite = "role_connections.write",
        /// Control the local client (RPC).
        Rpc = "rpc",
        /// Update the activity of the user (RPC).
        RpcActivitiesWrite = "rpc.activities.write",
        /// Read notifications (RPC).
        RpcNotificationsRead = "rpc.notifications.read",
        /// Read voice settings (RPC).
        RpcVoiceRead = "rpc.voice.read",
        /// Update voice settings (RPC).
        RpcVoiceWrite = "rpc.voice.write",
        /// Connect to voice for the user.
        Voice = "voice",
        /// Make an incoming webhook.
        WebhookIncoming = "webhook.incoming",
        /// Use the social layer of the Discord Social SDK.
        SdkSocialLayer = "sdk.social_layer",
        /// Use the social layer presence of the Discord Social SDK.
        SdkSocialLayerPresence = "sdk.social_layer_presence",
        /// Update application identity profiles.
        ApplicationIdentitiesWrite = "application_identities.write",
    }
}
