using Turboapi.Places;
using Turboapi.Places.Core;

// Standalone Places host: anonymous reference-data service (search + reverse
// geocoding). No auth scheme, no NATS — the module is the pure query side;
// ingestion runs as a separate job. The per-client rate limit + 429s are owned
// by the module (AddPlacesModule), so this host just activates the middleware.
var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

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
