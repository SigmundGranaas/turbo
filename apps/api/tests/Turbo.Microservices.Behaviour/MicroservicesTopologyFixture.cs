using System.Net.Http.Headers;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Testcontainers.PostgreSql;
using DotNet.Testcontainers.Containers;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Activities;
using Turboapi.Activities.BackcountrySki;
using Turboapi.Activities.Fishing;
using Turboapi.Activities.Freediving;
using Turboapi.Activities.Hiking;
using Turboapi.Activities.Packrafting;
using Turboapi.Activities.XcSki;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Collections.data;
using Turboapi.Geo.domain.query.model;
using Turboapi.Sharing.data;
using Turboapi.Tracks.data;
using Xunit;

namespace Turbo.Microservices.Behaviour;

/// <summary>
/// Boots all three microservice host projects (Turbo.Host.Auth,
/// Turbo.Host.Geo, Turbo.Host.Tracks) as separate
/// <see cref="WebApplicationFactory{TEntryPoint}"/> instances in the same
/// test process, sharing one Postgres container with three databases and
/// one NATS container. This matches the production microservice deploy:
/// three independent HTTP services that communicate through the broker
/// only, never directly in-process.
/// </summary>
public sealed class MicroservicesTopologyFixture : IAsyncLifetime
{
    private readonly PostgreSqlContainer _postgres = TurboTestContainers.PostgresWithPostGis();
    private readonly IContainer _nats = TurboTestContainers.NatsJetStream();

    private WebApplicationFactory<Turbo.Host.Auth.AuthHostProgram>? _authFactory;
    private WebApplicationFactory<Turbo.Host.Tracks.TracksHostProgram>? _tracksFactory;
    private WebApplicationFactory<Turbo.Host.Geo.GeoHostProgram>? _geoFactory;
    private WebApplicationFactory<Turbo.Host.Collections.CollectionsHostProgram>? _collectionsFactory;
    private WebApplicationFactory<Turbo.Host.Sharing.SharingHostProgram>? _sharingFactory;
    private WebApplicationFactory<Turbo.Host.Activities.ActivitiesHostProgram>? _activitiesFactory;

    private TurboJwtIssuer? _jwt;

    public HttpClient AuthClient => _authFactory!.CreateClient();
    public HttpClient TracksClient => _tracksFactory!.CreateClient();
    public HttpClient GeoClient => _geoFactory!.CreateClient();
    public HttpClient CollectionsClient => _collectionsFactory!.CreateClient();
    public HttpClient SharingClient => _sharingFactory!.CreateClient();
    public HttpClient ActivitiesClient => _activitiesFactory!.CreateClient();

    /// <summary>Issues a test JWT signed with the same key all hosts validate.</summary>
    public string IssueJwt(Guid userId) => _jwt!.Issue(userId);

    /// <summary>HTTP client preauthenticated as the given user.</summary>
    public HttpClient CollectionsClientAs(Guid userId) => Authed(_collectionsFactory!.CreateClient(), userId);
    public HttpClient SharingClientAs(Guid userId) => Authed(_sharingFactory!.CreateClient(), userId);
    public HttpClient GeoClientAs(Guid userId) => Authed(_geoFactory!.CreateClient(), userId);
    public HttpClient TracksClientAs(Guid userId) => Authed(_tracksFactory!.CreateClient(), userId);

    private HttpClient Authed(HttpClient client, Guid userId)
    {
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", IssueJwt(userId));
        return client;
    }

    public async Task InitializeAsync()
    {
        await Task.WhenAll(_postgres.StartAsync(), _nats.StartAsync());

        var baseConn = _postgres.GetConnectionString();
        var authConn = RepoLayout.WithDatabase(baseConn, "auth");
        var tracksConn = RepoLayout.WithDatabase(baseConn, "tracks");
        var geoConn = RepoLayout.WithDatabase(baseConn, "geo");
        var collectionsConn = RepoLayout.WithDatabase(baseConn, "collections");
        var sharingConn = RepoLayout.WithDatabase(baseConn, "sharing");
        // The activities host now owns ONE database with per-kind
        // Postgres schemas (fishing, hiking, …). The schema names are an
        // implementation detail owned by each module — this fixture only
        // hands the host the single connection string.
        var activitiesConn = RepoLayout.WithDatabase(baseConn, "activities");

        var natsUrl = TurboTestContainers.NatsUrl(_nats);
        _authFactory = BuildFactory<Turbo.Host.Auth.AuthHostProgram>(authConn, natsUrl, "Auth");
        _tracksFactory = BuildFactory<Turbo.Host.Tracks.TracksHostProgram>(tracksConn, natsUrl, "Tracks");
        _geoFactory = BuildFactory<Turbo.Host.Geo.GeoHostProgram>(geoConn, natsUrl, "Geo");
        _collectionsFactory = BuildFactory<Turbo.Host.Collections.CollectionsHostProgram>(collectionsConn, natsUrl, "Collections");
        // Sharing's host runs in this topology too: its sidecar subscribers
        // (cross-stream consumers on TURBO_COLLECTIONS / TURBO_GEO /
        // TURBO_TRACKS) are the seam the design relies on for the Resource
        // envelope to track payload-module entities over NATS.
        _sharingFactory = BuildFactory<Turbo.Host.Sharing.SharingHostProgram>(sharingConn, natsUrl, "Sharing");
        _activitiesFactory = BuildFactory<Turbo.Host.Activities.ActivitiesHostProgram>(activitiesConn, natsUrl, "Activities");

        // Capture the Jwt key from any one of the hosts — Test env loads
        // the same shared key into every host via appsettings.Test.json.
        _jwt = new TurboJwtIssuer(
            _authFactory.Services.GetRequiredService<Microsoft.Extensions.Configuration.IConfiguration>()["Jwt:Key"]
                ?? throw new InvalidOperationException("Jwt:Key not configured for Test environment"));

        // EF Core's MigrateAsync needs the target DB to exist; the helper
        // creates it lazily. Each host's Program.cs migrates at startup
        // already, but WebApplicationFactory doesn't execute that startup
        // path, so we run the migrations explicitly here.
        await _authFactory.Services.MigrateModuleDatabaseAsync<AuthDbContext>(authConn);
        await _tracksFactory.Services.MigrateModuleDatabaseAsync<TrackReadContext>(tracksConn);
        await _geoFactory.Services.MigrateModuleDatabaseAsync<LocationReadContext>(geoConn);
        await _collectionsFactory.Services.MigrateModuleDatabaseAsync<CollectionsReadContext>(collectionsConn);
        await _sharingFactory.Services.MigrateModuleDatabaseAsync<SharingReadContext>(sharingConn);
        await _activitiesFactory.Services.MigrateActivitiesSharedModuleAsync(activitiesConn);
        await _activitiesFactory.Services.MigrateFishingActivityModuleAsync(activitiesConn);
        await _activitiesFactory.Services.MigrateBackcountrySkiActivityModuleAsync(activitiesConn);
        await _activitiesFactory.Services.MigrateHikingActivityModuleAsync(activitiesConn);
        await _activitiesFactory.Services.MigrateXcSkiActivityModuleAsync(activitiesConn);
        await _activitiesFactory.Services.MigratePackraftingActivityModuleAsync(activitiesConn);
        await _activitiesFactory.Services.MigrateFreedivingActivityModuleAsync(activitiesConn);
    }

    public async Task DisposeAsync()
    {
        _authFactory?.Dispose();
        _tracksFactory?.Dispose();
        _geoFactory?.Dispose();
        _collectionsFactory?.Dispose();
        _sharingFactory?.Dispose();
        _activitiesFactory?.Dispose();
        await Task.WhenAll(_nats.DisposeAsync().AsTask(), _postgres.DisposeAsync().AsTask());
    }

    private static WebApplicationFactory<T> BuildFactory<T>(
        string connectionString, string natsUrl, string moduleConnStringName)
        where T : class
    {
        return new WebApplicationFactory<T>().WithWebHostBuilder(builder =>
        {
            builder.UseEnvironment("Test");
            builder.UseContentRoot(RepoLayout.HostContentRoot<T>());
            builder.UseSetting("Nats:Url", natsUrl);
            builder.UseSetting($"ConnectionStrings:{moduleConnStringName}", connectionString);
        });
    }

}

[CollectionDefinition("MicroservicesTopology")]
public sealed class MicroservicesTopologyCollection : ICollectionFixture<MicroservicesTopologyFixture> { }
