using Microsoft.EntityFrameworkCore;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Collections.domain.events;
using Turboapi.Geo.domain.events;
using Turboapi.Sharing.data;
using Turboapi.Sharing.domain;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.integration;
using Turboapi.Tracks.domain.events;

namespace Turboapi.Sharing;

/// <summary>
/// DI wiring for the Sharing service. Mirrors the per-module pattern used
/// by Collections / Geo / Tracks: register the DbContext, the access
/// control implementation, the outbox + idempotency plumbing, and any
/// controllers shipped by this assembly.
/// </summary>
public static class SharingModule
{
    public const string ConnectionStringName = "Sharing";

    public static IServiceCollection AddSharingModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);

        services.AddDbContext<SharingReadContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.EnableRetryOnFailure();
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<IAccessControl, EfAccessControl>();
        services.AddScoped<IFriendshipService, EfFriendshipService>();
        services.AddScoped<IGroupService, EfGroupService>();
        services.AddScoped<IGrantService, EfGrantService>();
        services.AddScoped<IShareInviteService, EfShareInviteService>();
        services.AddScoped<IResourceSyncService, EfResourceSyncService>();

        // Integration: subscribe to payload-module events and maintain the
        // Resource sidecar. New shareable types add another handler here.
        services.AddScoped<ResourceSidecar>();

        services.AddScoped<CollectionResourceSidecarHandler>();
        services.AddScoped<IEventHandler<CollectionCreated>>(sp =>
            sp.GetRequiredService<CollectionResourceSidecarHandler>());
        services.AddScoped<IEventHandler<CollectionDeleted>>(sp =>
            sp.GetRequiredService<CollectionResourceSidecarHandler>());

        services.AddScoped<MarkerResourceSidecarHandler>();
        services.AddScoped<IEventHandler<LocationCreated>>(sp =>
            sp.GetRequiredService<MarkerResourceSidecarHandler>());
        services.AddScoped<IEventHandler<LocationDeleted>>(sp =>
            sp.GetRequiredService<MarkerResourceSidecarHandler>());

        services.AddScoped<PathResourceSidecarHandler>();
        services.AddScoped<IEventHandler<TrackCreated>>(sp =>
            sp.GetRequiredService<PathResourceSidecarHandler>());
        services.AddScoped<IEventHandler<TrackDeleted>>(sp =>
            sp.GetRequiredService<PathResourceSidecarHandler>());

        services.AddScoped<IOutbox<SharingScope>, PgOutbox<SharingReadContext, SharingScope>>();
        services.AddScoped<IUnitOfWork<SharingScope>, PgUnitOfWork<SharingReadContext, SharingScope>>();
        services.AddScoped<IIdempotencyStore<SharingReadContext>, PgIdempotencyStore<SharingReadContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<SharingReadContext>>();

        services.AddControllers().AddApplicationPart(typeof(SharingModule).Assembly);

        return services;
    }

    /// <summary>
    /// Registers JetStream consumers on the Collections, Geo, and Tracks
    /// streams so the standalone Turbo.Host.Sharing process keeps the
    /// Resource sidecar in sync. The publishing services own their
    /// respective streams (TURBO_COLLECTIONS / TURBO_GEO / TURBO_TRACKS)
    /// and create them on boot; the Sharing host only binds consumers.
    ///
    /// In the modulith deployment, in-process subscribers handle the
    /// same events and these NATS registrations are skipped (the
    /// modulith host doesn't call this).
    /// </summary>
    public static IServiceCollection AddSharingNatsSubscribers(this IServiceCollection services)
    {
        // Collections sidecar
        services.AddNatsSubscriberOnStream<CollectionCreated>(
            "TURBO_COLLECTIONS", "turbo.collections.CollectionCreated", "sharing-collection-created");
        services.AddNatsSubscriberOnStream<CollectionDeleted>(
            "TURBO_COLLECTIONS", "turbo.collections.CollectionDeleted", "sharing-collection-deleted");
        // Markers sidecar
        services.AddNatsSubscriberOnStream<LocationCreated>(
            "TURBO_GEO", "turbo.geo.LocationCreated", "sharing-location-created");
        services.AddNatsSubscriberOnStream<LocationDeleted>(
            "TURBO_GEO", "turbo.geo.LocationDeleted", "sharing-location-deleted");
        // Paths sidecar
        services.AddNatsSubscriberOnStream<TrackCreated>(
            "TURBO_TRACKS", "turbo.tracks.TrackCreated", "sharing-track-created");
        services.AddNatsSubscriberOnStream<TrackDeleted>(
            "TURBO_TRACKS", "turbo.tracks.TrackDeleted", "sharing-track-deleted");
        return services;
    }

    private static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
