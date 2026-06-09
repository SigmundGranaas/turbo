using Turboapi.Places;

// Standalone Places host: anonymous reference-data service (search + reverse
// geocoding) behind the YARP gateway. No auth scheme, no NATS — the module is
// the pure query side; ingestion runs as a separate job.
var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddPlacesModule(builder.Configuration);

var app = builder.Build();
await app.Services.InitializePlacesModuleAsync(
    PlacesModule.ResolveConnectionString(builder.Configuration));

app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok"));
app.Run();

namespace Turbo.Host.Places
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class PlacesHostProgram;
}
