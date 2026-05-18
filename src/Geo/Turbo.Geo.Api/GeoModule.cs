using Microsoft.EntityFrameworkCore;
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
