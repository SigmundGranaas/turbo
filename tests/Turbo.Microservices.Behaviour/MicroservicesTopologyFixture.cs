using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Testcontainers.PostgreSql;
using DotNet.Testcontainers.Containers;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
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

    public HttpClient AuthClient => _authFactory!.CreateClient();
    public HttpClient TracksClient => _tracksFactory!.CreateClient();
    public HttpClient GeoClient => _geoFactory!.CreateClient();
    public HttpClient CollectionsClient => _collectionsFactory!.CreateClient();

    public async Task InitializeAsync()
    {
        await Task.WhenAll(_postgres.StartAsync(), _nats.StartAsync());

        var baseConn = _postgres.GetConnectionString();
        var authConn = RepoLayout.WithDatabase(baseConn, "auth");
        var tracksConn = RepoLayout.WithDatabase(baseConn, "tracks");
        var geoConn = RepoLayout.WithDatabase(baseConn, "geo");
        var collectionsConn = RepoLayout.WithDatabase(baseConn, "collections");

        var natsUrl = TurboTestContainers.NatsUrl(_nats);
        _authFactory = BuildFactory<Turbo.Host.Auth.AuthHostProgram>(authConn, natsUrl, "Auth");
        _tracksFactory = BuildFactory<Turbo.Host.Tracks.TracksHostProgram>(tracksConn, natsUrl, "Tracks");
        _geoFactory = BuildFactory<Turbo.Host.Geo.GeoHostProgram>(geoConn, natsUrl, "Geo");
        _collectionsFactory = BuildFactory<Turbo.Host.Collections.CollectionsHostProgram>(collectionsConn, natsUrl, "Collections");

        // EF Core's MigrateAsync needs the target DB to exist; the helper
        // creates it lazily. Each host's Program.cs migrates at startup
        // already, but WebApplicationFactory doesn't execute that startup
        // path, so we run the migrations explicitly here.
        await _authFactory.Services.MigrateModuleDatabaseAsync<AuthDbContext>(authConn);
        await _tracksFactory.Services.MigrateModuleDatabaseAsync<TrackReadContext>(tracksConn);
        await _geoFactory.Services.MigrateModuleDatabaseAsync<LocationReadContext>(geoConn);
        await _collectionsFactory.Services.MigrateModuleDatabaseAsync<CollectionsReadContext>(collectionsConn);
    }

    public async Task DisposeAsync()
    {
        _authFactory?.Dispose();
        _tracksFactory?.Dispose();
        _geoFactory?.Dispose();
        _collectionsFactory?.Dispose();
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
