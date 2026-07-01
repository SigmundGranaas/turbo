using Microsoft.Extensions.Configuration;
using Turbo.Hosting.Postgres;
using Turboapi.Places;
using Turboapi.Places.Core;

// Standalone Places host: anonymous reference-data service (search + reverse
// geocoding). No auth scheme, no NATS — the module is the pure query side;
// ingestion runs as a separate job. The per-client rate limit + 429s are owned
// by the module (AddPlacesModule), so this host just activates the middleware.
var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

// Cap the Npgsql pool size on the Places connection string. Places shares the
// turbo-db CNPG instance (max_connections 100) with the modulith, and Npgsql
// defaults to 100 connections per string — so leave headroom rather than let a
// single read service claim the whole server. DB_MAX_POOL_SIZE tunes the
// ceiling (default 15); an explicit "Maximum Pool Size" in the string wins.
var maxPoolSize = builder.Configuration.GetValue<int?>("DB_MAX_POOL_SIZE") ?? 15;
var cappedConnStrings = builder.Configuration.GetSection("ConnectionStrings")
    .GetChildren()
    .Where(c => !string.IsNullOrWhiteSpace(c.Value))
    .ToDictionary(
        c => $"ConnectionStrings:{c.Key}",
        c => (string?)NpgsqlPoolSizing.WithMaxPoolSize(c.Value, maxPoolSize));
if (cappedConnStrings.Count > 0)
{
    builder.Configuration.AddInMemoryCollection(cappedConnStrings);
}

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddPlacesModule(builder.Configuration);

var app = builder.Build();
await app.Services.InitializePlacesModuleAsync(
    PlacesModule.ResolveConnectionString(builder.Configuration));

app.UseRateLimiter();
app.MapControllers();

// Liveness: process is up.
app.MapGet("/healthz", () => Results.Ok("ok"));

// Readiness: the DB is reachable and the schema is queryable. Deliberately
// does NOT require a published dataset — a fresh deploy must roll out before
// its first ingest, and an empty dataset 404s per request rather than failing
// readiness. 503 only when the store can't be queried at all.
app.MapGet("/readyz", async (IPlaceStore store, CancellationToken ct) =>
{
    try
    {
        var (places, _, version) = await store.StatsAsync(ct);
        return Results.Ok(new { status = "ready", datasetVersion = version, places });
    }
    catch
    {
        return Results.StatusCode(StatusCodes.Status503ServiceUnavailable);
    }
});

app.Run();

namespace Turbo.Host.Places
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class PlacesHostProgram;
}
