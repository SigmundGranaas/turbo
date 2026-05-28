using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection.Extensions;
using NetTopologySuite.Geometries;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Sharing;
using Turboapi.Sharing.data;
using Turboapi.Tracks.controller;
using Turboapi.Tracks.data;
using Turboapi.Tracks.domain.events;
using Turboapi.Tracks.domain.handler;
using Turboapi.Tracks.domain.query;

namespace Turboapi.Tracks;

/// <summary>
/// Composition entry point for the Tracks module. Wires DbContext, repos,
/// command/query handlers, the asynchronous read-model projector, and the
/// outbox dispatcher. Messaging transport and auth scheme are left to the host.
/// </summary>
public static class TracksModule
{
    public const string ConnectionStringName = "Tracks";

    public static IServiceCollection AddTracksModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);

        services.AddDbContext<TrackReadContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<ITrackWriteRepository, EfTrackWriteRepository>();
        services.AddScoped<ITrackReadRepository, EfTrackWriteRepository.EfTrackReadRepository>();

        services.AddScoped<TrackCreatedHandler>();
        services.AddScoped<TrackUpdatedHandler>();
        services.AddScoped<TrackDeletedHandler>();
        services.AddScoped<IEventHandler<TrackCreated>>(sp => sp.GetRequiredService<TrackCreatedHandler>());
        services.AddScoped<IEventHandler<TrackUpdated>>(sp => sp.GetRequiredService<TrackUpdatedHandler>());
        services.AddScoped<IEventHandler<TrackDeleted>>(sp => sp.GetRequiredService<TrackDeletedHandler>());

        services.AddScoped<GetTrackByIdHandler>();
        services.AddScoped<GetUserTracksHandler>();
        services.AddScoped<GetTracksChangedSinceHandler>();
        services.AddScoped<CreateTrackHandler>();
        services.AddScoped<UpdateTrackHandler>();
        services.AddScoped<DeleteTrackHandler>();

        services.AddScoped<GeometryFactory>();

        services.AddScoped<IOutbox<TracksScope>, PgOutbox<TrackReadContext, TracksScope>>();
        services.AddScoped<IUnitOfWork<TracksScope>, PgUnitOfWork<TrackReadContext, TracksScope>>();
        services.AddScoped<IIdempotencyStore<TrackReadContext>, PgIdempotencyStore<TrackReadContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<TrackReadContext>>();

        services.AddControllers().AddApplicationPart(typeof(TracksController).Assembly);

        return services;
    }

    /// <summary>
    /// Registers an IAccessControl implementation backed by a read view
    /// of the Sharing schema. Required by the standalone Tracks host so
    /// its read/write handlers can consult grants. In the modulith
    /// deploy, Sharing's own module already registers IAccessControl.
    /// </summary>
    public static IServiceCollection AddTracksAccessControl(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var sharingConn = configuration.GetConnectionString("Sharing")
            ?? throw new InvalidOperationException(
                "ConnectionStrings:Sharing must be configured so Tracks can consult IAccessControl.");

        services.AddDbContext<SharingReadContext>(o =>
            o.UseNpgsql(sharingConn, npgsql => npgsql.EnableRetryOnFailure()));
        services.TryAddScoped<IAccessControl, EfAccessControl>();
        return services;
    }

    public static IServiceCollection AddTracksNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<TrackCreated>("turbo.tracks.TrackCreated", "tracks-track-created");
        services.AddNatsSubscriber<TrackUpdated>("turbo.tracks.TrackUpdated", "tracks-track-updated");
        services.AddNatsSubscriber<TrackDeleted>("turbo.tracks.TrackDeleted", "tracks-track-deleted");
        return services;
    }

    private static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
