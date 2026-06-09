using Turboapi.Places.controller;
using Turboapi.Places.Core;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places;

/// <summary>
/// Composition entry point for the Places module: the read-only reference-data
/// service (search + reverse geocoding) over the owned Kartverket datasets.
/// Unlike the payload modules there is no aggregate/outbox — the writer is the
/// ingestion job; this module is purely the query side.
/// </summary>
public static class PlacesModule
{
    public const string ConnectionStringName = "Places";

    public static IServiceCollection AddPlacesModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);

        // Stateless services over a read-only dataset: singletons, no scopes.
        services.AddSingleton<IPlaceStore>(_ => new PgPlaceStore(connectionString));
        services.AddSingleton<ReverseGeocodeService>();
        services.AddSingleton<SearchService>();

        services.AddControllers().AddApplicationPart(typeof(PlacesController).Assembly);

        return services;
    }

    /// <summary>
    /// Startup hook (the Places analogue of <c>MigrateModuleDatabaseAsync</c>):
    /// creates the database if missing and applies the idempotent schema DDL.
    /// </summary>
    public static Task InitializePlacesModuleAsync(
        this IServiceProvider _, string connectionString, CancellationToken ct = default)
        => PlacesDatabaseInitializer.InitializeAsync(connectionString, ct);

    public static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
