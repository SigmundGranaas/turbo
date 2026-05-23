using Turbo.Messaging.Nats;
using Turboapi.Activities;
using Turboapi.Activities.BackcountrySki;
using Turboapi.Activities.Fishing;
using Turboapi.Activities.Freediving;
using Turboapi.Activities.Hiking;
using Turboapi.Activities.Packrafting;
using Turboapi.Activities.XcSki;
using TurboAuthentication.Extensions;

var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();
builder.Services.AddAuthorization();

// Auth scheme — every activities endpoint requires a JWT. The token issuer
// is the Auth host; this service consumes the same signed token.
builder.Services.AddTurboAuth(builder.Configuration);

// One connection string for the whole activities module — each kind
// isolates itself with a Postgres schema (fishing, hiking, …) inside
// the shared database. The host never sees those schema names.
var activitiesConn = builder.Configuration.GetConnectionString("Activities")
    ?? throw new InvalidOperationException("ConnectionStrings:Activities is not configured");

builder.Services.AddActivitiesSharedModule(builder.Configuration, activitiesConn);
builder.Services.AddFishingActivityModule(activitiesConn);
builder.Services.AddBackcountrySkiActivityModule(activitiesConn);
builder.Services.AddHikingActivityModule(activitiesConn);
builder.Services.AddXcSkiActivityModule(activitiesConn);
builder.Services.AddPackraftingActivityModule(activitiesConn);
builder.Services.AddFreedivingActivityModule(activitiesConn);

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

// Each module owns its schema + migrations history table; the host
// just hands them the connection string. Order doesn't matter — no
// cross-schema FKs.
await app.Services.MigrateActivitiesSharedModuleAsync(activitiesConn);
await app.Services.MigrateFishingActivityModuleAsync(activitiesConn);
await app.Services.MigrateBackcountrySkiActivityModuleAsync(activitiesConn);
await app.Services.MigrateHikingActivityModuleAsync(activitiesConn);
await app.Services.MigrateXcSkiActivityModuleAsync(activitiesConn);
await app.Services.MigratePackraftingActivityModuleAsync(activitiesConn);
await app.Services.MigrateFreedivingActivityModuleAsync(activitiesConn);

if (!app.Environment.IsDevelopment()) app.UseHttpsRedirection();
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
