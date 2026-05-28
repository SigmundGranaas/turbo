using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection.Extensions;
using NetTopologySuite.Geometries;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Geo.controller;
using Turboapi.Geo.data;
using Turboapi.Geo.domain.events;
using Turboapi.Geo.domain.handler;
using Turboapi.Geo.domain.query;
using Turboapi.Geo.domain.query.model;
using Turboapi.Sharing;
using Turboapi.Sharing.data;

namespace Turboapi.Geo;

/// <summary>
/// Composition entry point for the Geo module. Wires DbContext, repos,
/// command/query handlers, the synchronous read-model projector, and the
/// outbox dispatcher. The messaging transport and auth scheme are left to
/// the host.
/// </summary>
public static class GeoModule
{
    public const string ConnectionStringName = "Geo";

    public static IServiceCollection AddGeoModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);

        services.AddDbContext<LocationReadContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<ILocationWriteRepository, EfLocationWriteRepository>();
        services.AddScoped<ILocationReadRepository, EfLocationWriteRepository.EfLocationReadRepository>();

        services.AddScoped<LocationCreatedHandler>();
        services.AddScoped<LocationUpdatedHandler>();
        services.AddScoped<LocationDeletedHandler>();
        services.AddScoped<IEventHandler<LocationCreated>>(sp => sp.GetRequiredService<LocationCreatedHandler>());
        services.AddScoped<IEventHandler<LocationUpdated>>(sp => sp.GetRequiredService<LocationUpdatedHandler>());
        services.AddScoped<IEventHandler<LocationDeleted>>(sp => sp.GetRequiredService<LocationDeletedHandler>());

        services.AddScoped<GetLocationByIdHandler>();
        services.AddScoped<GetLocationsInExtentHandler>();
        services.AddScoped<GetLocationsChangedSinceHandler>();
        services.AddScoped<CreateLocationHandler>();
        services.AddScoped<DeleteLocationHandler>();
        services.AddScoped<UpdateLocationHandler>();

        services.AddScoped<GeometryFactory>();

        services.AddScoped<IOutbox<GeoScope>, PgOutbox<LocationReadContext, GeoScope>>();
        services.AddScoped<IUnitOfWork<GeoScope>, PgUnitOfWork<LocationReadContext, GeoScope>>();
        services.AddScoped<IIdempotencyStore<LocationReadContext>, PgIdempotencyStore<LocationReadContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<LocationReadContext>>();

        services.AddControllers().AddApplicationPart(typeof(LocationsController).Assembly);

        return services;
    }

    /// <summary>
    /// Registers an IAccessControl implementation backed by a read view
    /// of the Sharing schema. Required by the standalone Geo host so
    /// read/write handlers can consult grants. In the modulith deploy,
    /// Sharing's own module already registers IAccessControl.
    /// </summary>
    public static IServiceCollection AddGeoAccessControl(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var sharingConn = configuration.GetConnectionString("Sharing")
            ?? throw new InvalidOperationException(
                "ConnectionStrings:Sharing must be configured so Geo can consult IAccessControl.");

        services.AddDbContext<SharingReadContext>(o =>
            o.UseNpgsql(sharingConn, npgsql => npgsql.EnableRetryOnFailure()));
        services.TryAddScoped<IAccessControl, EfAccessControl>();
        return services;
    }

    /// <summary>
    /// Registers Geo's projection subscribers on the NATS subjects the
    /// outbox dispatcher produces. Used by the microservice host.
    /// </summary>
    public static IServiceCollection AddGeoNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<LocationCreated>("turbo.geo.LocationCreated", "geo-location-created");
        services.AddNatsSubscriber<LocationUpdated>("turbo.geo.LocationUpdated", "geo-location-updated");
        services.AddNatsSubscriber<LocationDeleted>("turbo.geo.LocationDeleted", "geo-location-deleted");
        return services;
    }

    private static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
