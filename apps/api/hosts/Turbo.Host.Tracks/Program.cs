using Turbo.Hosting.Postgres;
using Turbo.Messaging.Nats;
using Turboapi.Tracks;
using Turboapi.Tracks.data;
using TurboAuthentication.Extensions;

var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddAuthorization();

builder.Services.AddTurboAuth(builder.Configuration);
builder.Services.AddTracksModule(builder.Configuration);
// Standalone Tracks host: wire IAccessControl against the Sharing schema
// so read/write handlers can consult grants.
builder.Services.AddTracksAccessControl(builder.Configuration);

builder.Services.AddNatsMessaging(o =>
{
    o.Url = builder.Configuration["Nats:Url"] ?? "nats://localhost:4222";
    o.StreamName = "TURBO_TRACKS";
    o.Subjects = ["turbo.tracks.>"];
    o.SubjectPrefix = "turbo.tracks";
});
builder.Services.AddTracksNatsSubscribers();

var app = builder.Build();
await app.Services.MigrateModuleDatabaseAsync<TrackReadContext>(
    builder.Configuration.GetConnectionString("Tracks")
        ?? throw new InvalidOperationException("ConnectionStrings:Tracks is not configured"));
if (!app.Environment.IsDevelopment()) app.UseHttpsRedirection();
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok")).AllowAnonymous();
app.Run();

namespace Turbo.Host.Tracks
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class TracksHostProgram;
}
