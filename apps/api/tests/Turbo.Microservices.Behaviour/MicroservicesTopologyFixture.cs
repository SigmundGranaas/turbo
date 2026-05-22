using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Testcontainers.PostgreSql;
using DotNet.Testcontainers.Containers;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Activities.BackcountrySki.data;
using Turboapi.Activities.data;
using Turboapi.Activities.Fishing.data;
using Turboapi.Activities.Freediving.data;
using Turboapi.Activities.Hiking.data;
using Turboapi.Activities.Packrafting.data;
using Turboapi.Activities.XcSki.data;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Collections.data;
using Turboapi.Geo.domain.query.model;
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
    private WebApplicationFactory<Turbo.Host.Activities.ActivitiesHostProgram>? _activitiesFactory;

    public HttpClient AuthClient => _authFactory!.CreateClient();
    public HttpClient TracksClient => _tracksFactory!.CreateClient();
    public HttpClient GeoClient => _geoFactory!.CreateClient();
    public HttpClient CollectionsClient => _collectionsFactory!.CreateClient();
    public HttpClient ActivitiesClient => _activitiesFactory!.CreateClient();

    public async Task InitializeAsync()
    {
        await Task.WhenAll(_postgres.StartAsync(), _nats.StartAsync());

        var baseConn = _postgres.GetConnectionString();
        var authConn = RepoLayout.WithDatabase(baseConn, "auth");
        var tracksConn = RepoLayout.WithDatabase(baseConn, "tracks");
        var geoConn = RepoLayout.WithDatabase(baseConn, "geo");
        var collectionsConn = RepoLayout.WithDatabase(baseConn, "collections");
        // The activities host owns seven databases — one cross-kind summary
        // store plus one per kind. Matching the modulith fixture so the same
        // user-visible contract holds across both deploy shapes.
        var activitiesConn = RepoLayout.WithDatabase(baseConn, "activities");
        var actFishingConn = RepoLayout.WithDatabase(baseConn, "activities_fishing");
        var actBcSkiConn = RepoLayout.WithDatabase(baseConn, "activities_backcountry_ski");
        var actHikingConn = RepoLayout.WithDatabase(baseConn, "activities_hiking");
        var actXcSkiConn = RepoLayout.WithDatabase(baseConn, "activities_xc_ski");
        var actPackraftingConn = RepoLayout.WithDatabase(baseConn, "activities_packrafting");
        var actFreedivingConn = RepoLayout.WithDatabase(baseConn, "activities_freediving");

        var natsUrl = TurboTestContainers.NatsUrl(_nats);
        _authFactory = BuildFactory<Turbo.Host.Auth.AuthHostProgram>(authConn, natsUrl, "Auth");
        _tracksFactory = BuildFactory<Turbo.Host.Tracks.TracksHostProgram>(tracksConn, natsUrl, "Tracks");
        _geoFactory = BuildFactory<Turbo.Host.Geo.GeoHostProgram>(geoConn, natsUrl, "Geo");
        _collectionsFactory = BuildFactory<Turbo.Host.Collections.CollectionsHostProgram>(collectionsConn, natsUrl, "Collections");
        _activitiesFactory = BuildActivitiesFactory(natsUrl, new Dictionary<string, string>
        {
            ["ConnectionStrings:Activities"] = activitiesConn,
            ["ConnectionStrings:ActivitiesFishing"] = actFishingConn,
            ["ConnectionStrings:ActivitiesBackcountrySki"] = actBcSkiConn,
            ["ConnectionStrings:ActivitiesHiking"] = actHikingConn,
            ["ConnectionStrings:ActivitiesXcSki"] = actXcSkiConn,
            ["ConnectionStrings:ActivitiesPackrafting"] = actPackraftingConn,
            ["ConnectionStrings:ActivitiesFreediving"] = actFreedivingConn,
        });

        // EF Core's MigrateAsync needs the target DB to exist; the helper
        // creates it lazily. Each host's Program.cs migrates at startup
        // already, but WebApplicationFactory doesn't execute that startup
        // path, so we run the migrations explicitly here.
        await _authFactory.Services.MigrateModuleDatabaseAsync<AuthDbContext>(authConn);
        await _tracksFactory.Services.MigrateModuleDatabaseAsync<TrackReadContext>(tracksConn);
        await _geoFactory.Services.MigrateModuleDatabaseAsync<LocationReadContext>(geoConn);
        await _collectionsFactory.Services.MigrateModuleDatabaseAsync<CollectionsReadContext>(collectionsConn);
        await _activitiesFactory.Services.MigrateModuleDatabaseAsync<ActivitySummariesContext>(activitiesConn);
        await _activitiesFactory.Services.MigrateModuleDatabaseAsync<FishingContext>(actFishingConn);
        await _activitiesFactory.Services.MigrateModuleDatabaseAsync<BackcountrySkiContext>(actBcSkiConn);
        await _activitiesFactory.Services.MigrateModuleDatabaseAsync<HikingContext>(actHikingConn);
        await _activitiesFactory.Services.MigrateModuleDatabaseAsync<XcSkiContext>(actXcSkiConn);
        await _activitiesFactory.Services.MigrateModuleDatabaseAsync<PackraftingContext>(actPackraftingConn);
        await _activitiesFactory.Services.MigrateModuleDatabaseAsync<FreedivingContext>(actFreedivingConn);
    }

    public async Task DisposeAsync()
    {
        _authFactory?.Dispose();
        _tracksFactory?.Dispose();
        _geoFactory?.Dispose();
        _collectionsFactory?.Dispose();
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

    /// <summary>
    /// The activities host owns seven connection strings — one for the
    /// cross-kind summaries DB plus one per kind. Per-key UseSetting calls
    /// keep the same env-var override surface the host expects.
    /// </summary>
    private static WebApplicationFactory<Turbo.Host.Activities.ActivitiesHostProgram>
        BuildActivitiesFactory(string natsUrl, IDictionary<string, string> connectionStrings)
    {
        return new WebApplicationFactory<Turbo.Host.Activities.ActivitiesHostProgram>()
            .WithWebHostBuilder(builder =>
            {
                builder.UseEnvironment("Test");
                builder.UseContentRoot(
                    RepoLayout.HostContentRoot<Turbo.Host.Activities.ActivitiesHostProgram>());
                builder.UseSetting("Nats:Url", natsUrl);
                foreach (var (key, value) in connectionStrings)
                {
                    builder.UseSetting(key, value);
                }
            });
    }
}

[CollectionDefinition("MicroservicesTopology")]
public sealed class MicroservicesTopologyCollection : ICollectionFixture<MicroservicesTopologyFixture> { }
