using Microsoft.EntityFrameworkCore;
using Turbo.Messaging;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Collections.domain.events;
using Turboapi.Sharing.data;
using Turboapi.Sharing.domain;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.integration;

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
        services.AddScoped<CollectionResourceSidecarHandler>();
        services.AddScoped<IEventHandler<CollectionCreated>>(sp =>
            sp.GetRequiredService<CollectionResourceSidecarHandler>());
        services.AddScoped<IEventHandler<CollectionDeleted>>(sp =>
            sp.GetRequiredService<CollectionResourceSidecarHandler>());

        services.AddScoped<IOutbox<SharingScope>, PgOutbox<SharingReadContext, SharingScope>>();
        services.AddScoped<IUnitOfWork<SharingScope>, PgUnitOfWork<SharingReadContext, SharingScope>>();
        services.AddScoped<IIdempotencyStore<SharingReadContext>, PgIdempotencyStore<SharingReadContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<SharingReadContext>>();

        services.AddControllers().AddApplicationPart(typeof(SharingModule).Assembly);

        return services;
    }

    private static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
