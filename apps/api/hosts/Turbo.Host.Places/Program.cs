using System.Threading.RateLimiting;
using Microsoft.AspNetCore.RateLimiting;
using Turboapi.Places;

// Standalone Places host: anonymous reference-data service (search + reverse
// geocoding) behind the YARP gateway. No auth scheme, no NATS — the module is
// the pure query side; ingestion runs as a separate job.
var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddPlacesModule(builder.Configuration);

// Defense-in-depth rate limit (the gateway is the primary guard). A per-IP
// fixed window; disabled when Places:RateLimitPermitPerWindow <= 0.
var permit = builder.Configuration.GetValue("Places:RateLimitPermitPerWindow", 600);
var windowSeconds = builder.Configuration.GetValue("Places:RateLimitWindowSeconds", 60);
if (permit > 0)
{
    builder.Services.AddRateLimiter(options =>
    {
        options.RejectionStatusCode = StatusCodes.Status429TooManyRequests;
        options.GlobalLimiter = PartitionedRateLimiter.Create<HttpContext, string>(ctx =>
            RateLimitPartition.GetFixedWindowLimiter(
                ctx.Connection.RemoteIpAddress?.ToString() ?? "anon",
                _ => new FixedWindowRateLimiterOptions
                {
                    PermitLimit = permit,
                    Window = TimeSpan.FromSeconds(windowSeconds),
                    QueueLimit = 0,
                }));
    });
}

var app = builder.Build();
await app.Services.InitializePlacesModuleAsync(
    PlacesModule.ResolveConnectionString(builder.Configuration));

if (permit > 0) app.UseRateLimiter();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok"));
app.Run();

namespace Turbo.Host.Places
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class PlacesHostProgram;
}
