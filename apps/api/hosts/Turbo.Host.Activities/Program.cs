using Turbo.Hosting.Postgres;
using Turbo.Messaging.Nats;
using Turboapi.Activities;
using Turboapi.Activities.BackcountrySki;
using Turboapi.Activities.BackcountrySki.data;
using Turboapi.Activities.data;
using Turboapi.Activities.Fishing;
using Turboapi.Activities.Fishing.data;
using Turboapi.Activities.Freediving;
using Turboapi.Activities.Freediving.data;
using Turboapi.Activities.Hiking;
using Turboapi.Activities.Hiking.data;
using Turboapi.Activities.Packrafting;
using Turboapi.Activities.Packrafting.data;
using Turboapi.Activities.XcSki;
using Turboapi.Activities.XcSki.data;
using TurboAuthentication.Extensions;

var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddAuthorization();

// Auth scheme — every activities endpoint requires a JWT. The token issuer
// is the Auth host; this service consumes the same signed token.
builder.Services.AddTurboAuth(builder.Configuration);

// Shared activities module first (registers the cross-kind summaries
// projection + per-kind catalog), then each kind module.
builder.Services.AddActivitiesSharedModule(builder.Configuration);
builder.Services.AddFishingActivityModule(builder.Configuration);
builder.Services.AddBackcountrySkiActivityModule(builder.Configuration);
builder.Services.AddHikingActivityModule(builder.Configuration);
builder.Services.AddXcSkiActivityModule(builder.Configuration);
builder.Services.AddPackraftingActivityModule(builder.Configuration);
builder.Services.AddFreedivingActivityModule(builder.Configuration);

// NATS transport: one JetStream stream covers every per-kind subject —
// `turbo.activities.{fishing|backcountry_ski|hiking|xc_ski|packrafting|freediving}.*`.
// The per-kind subscribers bind durable consumers under that stream; the
// outbox dispatchers publish onto it.
builder.Services.AddNatsMessaging(o =>
{
    o.Url = builder.Configuration["Nats:Url"] ?? "nats://localhost:4222";
    o.StreamName = "TURBO_ACTIVITIES";
    o.Subjects = ["turbo.activities.>"];
    o.SubjectPrefix = "turbo.activities";
});

// Each kind exposes its own subscriber-wiring extension so durable names
// stay kind-scoped and a host can pick + choose which kinds to serve. The
// activities host serves all six.
builder.Services.AddFishingActivityNatsSubscribers();
builder.Services.AddBackcountrySkiActivityNatsSubscribers();
builder.Services.AddHikingActivityNatsSubscribers();
builder.Services.AddXcSkiActivityNatsSubscribers();
builder.Services.AddPackraftingActivityNatsSubscribers();
builder.Services.AddFreedivingActivityNatsSubscribers();

var app = builder.Build();

// Migrate every database the host owns. Order doesn't matter — each
// kind has its own schema and there are no FKs across kinds.
await app.Services.MigrateModuleDatabaseAsync<ActivitySummariesContext>(
    builder.Configuration.GetConnectionString("Activities")
        ?? throw new InvalidOperationException("ConnectionStrings:Activities is not configured"));
await app.Services.MigrateModuleDatabaseAsync<FishingContext>(
    builder.Configuration.GetConnectionString("ActivitiesFishing")
        ?? throw new InvalidOperationException("ConnectionStrings:ActivitiesFishing is not configured"));
await app.Services.MigrateModuleDatabaseAsync<BackcountrySkiContext>(
    builder.Configuration.GetConnectionString("ActivitiesBackcountrySki")
        ?? throw new InvalidOperationException("ConnectionStrings:ActivitiesBackcountrySki is not configured"));
await app.Services.MigrateModuleDatabaseAsync<HikingContext>(
    builder.Configuration.GetConnectionString("ActivitiesHiking")
        ?? throw new InvalidOperationException("ConnectionStrings:ActivitiesHiking is not configured"));
await app.Services.MigrateModuleDatabaseAsync<XcSkiContext>(
    builder.Configuration.GetConnectionString("ActivitiesXcSki")
        ?? throw new InvalidOperationException("ConnectionStrings:ActivitiesXcSki is not configured"));
await app.Services.MigrateModuleDatabaseAsync<PackraftingContext>(
    builder.Configuration.GetConnectionString("ActivitiesPackrafting")
        ?? throw new InvalidOperationException("ConnectionStrings:ActivitiesPackrafting is not configured"));
await app.Services.MigrateModuleDatabaseAsync<FreedivingContext>(
    builder.Configuration.GetConnectionString("ActivitiesFreediving")
        ?? throw new InvalidOperationException("ConnectionStrings:ActivitiesFreediving is not configured"));

app.UseHttpsRedirection();
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok")).AllowAnonymous();
app.Run();

namespace Turbo.Host.Activities
{
    /// <summary>Marker for WebApplicationFactory in tests.</summary>
    public class ActivitiesHostProgram;
}
