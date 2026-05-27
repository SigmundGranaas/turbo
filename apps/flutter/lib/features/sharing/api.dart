/// Public API for the sharing feature: incoming share-link handling
/// plus the identity-aware "social" sharing layer (resources, grants,
/// friendships, groups).
library;

// Legacy link-sharing surface — anonymous decode of `/share/m` and
// `/share/p` deep links into local copies. Kept as-is for backward
// compat; the design will gradually fold this into tracked link grants.
export 'data/pending_share_provider.dart'
    show pendingShareProvider, PendingShareNotifier;
export 'data/share_route_handler.dart' show ShareRouteHandler;
export 'data/share_link_listener_provider.dart' show shareLinkListenerProvider;
export 'widgets/shared_marker_preview_sheet.dart'
    show SharedMarkerPreviewSheet;
export 'widgets/shared_path_preview_sheet.dart' show SharedPathPreviewSheet;
export 'widgets/shared_payload_listener.dart' show SharedPayloadListener;

// Identity-aware sharing layer.
export 'models/sharing_models.dart'
    show
        ResourceVisibility,
        EffectiveRole,
        EffectiveRoleExt,
        GrantRole,
        ResourceEnvelope,
        ResourceSyncPage,
        FriendshipStatus,
        Friendship,
        FriendGroup,
        GroupMemberInfo,
        Grant,
        LinkGrant,
        ShareInvite;
export 'data/sharing_api_client.dart'
    show SharingApiClient, sharingApiClientProvider;
export 'data/role_cache_repository.dart'
    show RoleCacheRepository, roleCacheRepositoryProvider;
export 'providers/sharing_providers.dart'
    show
        canEditProvider,
        effectiveRoleProvider,
        acceptedFriendsProvider,
        allFriendshipsProvider,
        myGroupsProvider,
        grantsForResourceProvider;
export 'widgets/share_sheet.dart' show ShareSheet;
export 'widgets/friends_page.dart' show FriendsPage;
