using Turbo.Host.Modulith;
using Turbo.Hosting.Postgres;
using Turbo.Messaging.InProcess;
using Turboapi.Auth;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Collections;
using Turboapi.Collections.data;
using Turboapi.Geo;
using Turboapi.Geo.domain.query.model;
using Turboapi.Tracks;
using Turboapi.Tracks.data;
using Turboapi.Activities;
using Turboapi.Activities.data;
using Turboapi.Activities.Fishing;
using Turboapi.Activities.Fishing.data;
using Turboapi.Activities.BackcountrySki;
using Turboapi.Activities.BackcountrySki.data;
using Turboapi.Activities.Freediving;
using Turboapi.Activities.Freediving.data;
using Turboapi.Activities.Hiking;
using Turboapi.Activities.Hiking.data;
using Turboapi.Activities.Packrafting;
using Turboapi.Activities.Packrafting.data;
using Turboapi.Activities.XcSki;
using Turboapi.Activities.XcSki.data;

var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

builder.Services.AddEndpointsApiExplorer();

// All four modules in one process. AuthModule owns the Cookie+JwtBearer
// scheme; the other modules use it as the default authentication scheme
// (their [Authorize] attributes don't pin a specific scheme name).
builder.Services.AddAuthModule(builder.Configuration);
builder.Services.AddGeoModule(builder.Configuration);
builder.Services.AddTracksModule(builder.Configuration);
builder.Services.AddCollectionsModule(builder.Configuration);
builder.Services.AddActivitiesSharedModule(builder.Configuration);
builder.Services.AddFishingActivityModule(builder.Configuration);
builder.Services.AddBackcountrySkiActivityModule(builder.Configuration);
builder.Services.AddHikingActivityModule(builder.Configuration);
builder.Services.AddXcSkiActivityModule(builder.Configuration);
builder.Services.AddPackraftingActivityModule(builder.Configuration);
builder.Services.AddFreedivingActivityModule(builder.Configuration);

// In-process transport: outbox dispatchers publish here, the subscriber host
// drains the channel and resolves IEventHandler<T> in a fresh DI scope. No
// NATS, no broker — the read-model projection is end-to-end in-process.
builder.Services.AddInProcessMessaging();
// Subscriber registrations live in SubscriberWiring so the
// SubscriberCoverage architecture test can compare them against the
// set of IDomainEvent types in the modules.
builder.Services.AddTurboInProcessSubscribers();

var app = builder.Build();
await app.Services.MigrateModuleDatabaseAsync<AuthDbContext>(
    builder.Configuration.GetConnectionString("Auth")
        ?? throw new InvalidOperationException("ConnectionStrings:Auth is not configured"));
await app.Services.MigrateModuleDatabaseAsync<LocationReadContext>(
    builder.Configuration.GetConnectionString("Geo")
        ?? throw new InvalidOperationException("ConnectionStrings:Geo is not configured"));
await app.Services.MigrateModuleDatabaseAsync<TrackReadContext>(
    builder.Configuration.GetConnectionString("Tracks")
        ?? throw new InvalidOperationException("ConnectionStrings:Tracks is not configured"));
await app.Services.MigrateModuleDatabaseAsync<CollectionsReadContext>(
    builder.Configuration.GetConnectionString("Collections")
        ?? throw new InvalidOperationException("ConnectionStrings:Collections is not configured"));
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

app.UseRouting();
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok")).AllowAnonymous();
app.Run();

public partial class Program { }
