using Turbo.Hosting.Postgres;
using Turbo.Messaging.Nats;
using Turboapi.Sharing;
using Turboapi.Sharing.data;
using Turboapi.Sharing.integration;
using TurboAuthentication.Extensions;

var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddAuthorization();

builder.Services.AddTurboAuth(builder.Configuration);
builder.Services.AddSharingModule(builder.Configuration);

// Sharing's own outbox stream — the service publishes events of its own
// (future: GrantCreated, FriendshipAccepted, ...) on this stream. The
// per-registration stream overrides set on the cross-service subscribers
// route those consumers to the right peer streams.
builder.Services.AddNatsMessaging(o =>
{
    o.Url = builder.Configuration["Nats:Url"] ?? "nats://localhost:4222";
    o.StreamName = "TURBO_SHARING";
    o.Subjects = ["turbo.sharing.>"];
    o.SubjectPrefix = "turbo.sharing";
});
builder.Services.AddSharingNatsSubscribers();

var app = builder.Build();
await app.Services.MigrateModuleDatabaseAsync<SharingReadContext>(
    builder.Configuration.GetConnectionString("Sharing")
        ?? throw new InvalidOperationException("ConnectionStrings:Sharing is not configured"));

// One-shot backfill at startup. Skipped in the Test environment because
// the standalone test fixture replaces SharingReadContext's options after
// builder.Build() runs — creating a DbContext via the original options
// at this point leaves an internal logger reference that gets disposed
// when the fixture's ReplaceDbContext takes effect. The modulith host
// runs the backfill in production deploys via its own startup hook.
if (!app.Environment.IsEnvironment("Test"))
    await app.Services.BackfillSharingResourcesAsync(builder.Configuration);

if (!app.Environment.IsDevelopment()) app.UseHttpsRedirection();
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok")).AllowAnonymous();
app.Run();

namespace Turbo.Host.Sharing
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class SharingHostProgram;
}
