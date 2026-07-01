using Microsoft.Extensions.Configuration;
using Turbo.Host.Modulith;
using Turbo.Hosting.Postgres;
using Turbo.Messaging.InProcess;
using Turboapi.Auth;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Collections;
using Turboapi.Collections.data;
using Turboapi.Geo;
using Turboapi.Geo.domain.query.model;
using Turboapi.Sharing;
using Turboapi.Sharing.data;
using Turboapi.Sharing.integration;
using Turboapi.Tracks;
using Turboapi.Tracks.data;
using Turboapi.Activities;
using Turboapi.Activities.Fishing;
using Turboapi.Activities.BackcountrySki;
using Turboapi.Activities.Freediving;
using Turboapi.Activities.Hiking;
using Turboapi.Activities.Packrafting;
using Turboapi.Activities.XcSki;

var builder = WebApplication.CreateBuilder(args);
builder.Configuration.AddEnvironmentVariables();

// Cap the Npgsql pool size on every module connection string. Each module owns
// its own database (a distinct connection string) and Npgsql defaults to 100
// connections per string, so the modulith would otherwise open up to
// 100 × module-count against a single CNPG instance whose max_connections is
// 100 — connection exhaustion on the small single node. DB_MAX_POOL_SIZE tunes
// the per-module ceiling (default 15); an explicit "Maximum Pool Size" in a
// connection string always wins. Applied here at the composition root so every
// module reads the capped value via GetConnectionString.
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

// CORS for the browser SPAs that call this host cross-origin with credentials:
// the turbomap web app (kart.sandring.no) and the Vite dev server (5173). Both
// are same-site (sandring.no) so the Lax auth cookies flow; CORS gates only the
// browser's ability to read the response. Without this the modulith emits no
// ACAO header and every cross-origin call is blocked (dev hides it behind the
// Vite same-origin proxy). Mirrors Turbo.Host.Auth's standalone policy.
const string webAppCors = "WebApp";
builder.Services.AddCors(options =>
{
    options.AddPolicy(webAppCors, policy =>
    {
        policy
            .WithOrigins(
                "http://localhost:3000",
                "http://localhost:8080",
                "http://localhost:5173",
                "https://kart.sandring.no")
            .AllowAnyHeader()
            .AllowAnyMethod()
            .AllowCredentials();
    });
});

// All modules in one process. AuthModule owns the Cookie+JwtBearer
// scheme; the other modules use it as the default authentication scheme
// (their [Authorize] attributes don't pin a specific scheme name).
builder.Services.AddAuthModule(builder.Configuration);
builder.Services.AddSharingModule(builder.Configuration);
builder.Services.AddGeoModule(builder.Configuration);
builder.Services.AddTracksModule(builder.Configuration);
builder.Services.AddCollectionsModule(builder.Configuration);

// Activities is one DB with a schema per kind — host hands every kind
// the same connection string and the kind's registration extension
// pins its own schema + migrations history table internally.
var activitiesConn = builder.Configuration.GetConnectionString("Activities")
    ?? throw new InvalidOperationException("ConnectionStrings:Activities is not configured");
builder.Services.AddActivitiesSharedModule(builder.Configuration, activitiesConn);
builder.Services.AddFishingActivityModule(activitiesConn);
builder.Services.AddBackcountrySkiActivityModule(activitiesConn);
builder.Services.AddHikingActivityModule(activitiesConn);
builder.Services.AddXcSkiActivityModule(activitiesConn);
builder.Services.AddPackraftingActivityModule(activitiesConn);
builder.Services.AddFreedivingActivityModule(activitiesConn);

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
await app.Services.MigrateModuleDatabaseAsync<SharingReadContext>(
    builder.Configuration.GetConnectionString("Sharing")
        ?? throw new InvalidOperationException("ConnectionStrings:Sharing is not configured"));
await app.Services.MigrateModuleDatabaseAsync<LocationReadContext>(
    builder.Configuration.GetConnectionString("Geo")
        ?? throw new InvalidOperationException("ConnectionStrings:Geo is not configured"));
await app.Services.MigrateModuleDatabaseAsync<TrackReadContext>(
    builder.Configuration.GetConnectionString("Tracks")
        ?? throw new InvalidOperationException("ConnectionStrings:Tracks is not configured"));
await app.Services.MigrateModuleDatabaseAsync<CollectionsReadContext>(
    builder.Configuration.GetConnectionString("Collections")
        ?? throw new InvalidOperationException("ConnectionStrings:Collections is not configured"));
await app.Services.MigrateActivitiesSharedModuleAsync(activitiesConn);
await app.Services.MigrateFishingActivityModuleAsync(activitiesConn);
await app.Services.MigrateBackcountrySkiActivityModuleAsync(activitiesConn);
await app.Services.MigrateHikingActivityModuleAsync(activitiesConn);
await app.Services.MigrateXcSkiActivityModuleAsync(activitiesConn);
await app.Services.MigratePackraftingActivityModuleAsync(activitiesConn);
await app.Services.MigrateFreedivingActivityModuleAsync(activitiesConn);

// Backfill Resource envelopes for any pre-existing collections / markers /
// paths. Safe to re-run; rows already present are skipped. New entities
// flow through the event-driven sidecars in Turboapi.Sharing.integration.
await app.Services.BackfillSharingResourcesAsync(builder.Configuration);

app.UseRouting();
app.UseCors(webAppCors);
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapGet("/healthz", () => Results.Ok("ok")).AllowAnonymous();
app.Run();

public partial class Program { }
