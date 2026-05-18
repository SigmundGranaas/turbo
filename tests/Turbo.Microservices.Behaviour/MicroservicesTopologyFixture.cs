using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Testcontainers.PostgreSql;
using DotNet.Testcontainers.Containers;
using Turbo.Behaviour.Testing;
using Turbo.Hosting.Postgres;
using Turboapi.Activity.data;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Geo.domain.query.model;
using Xunit;

namespace Turbo.Microservices.Behaviour;

/// <summary>
/// Boots all three microservice host projects (Turbo.Host.Auth,
/// Turbo.Host.Activity, Turbo.Host.Geo) as separate
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
    private WebApplicationFactory<Turbo.Host.Activity.ActivityHostProgram>? _activityFactory;
    private WebApplicationFactory<Turbo.Host.Geo.GeoHostProgram>? _geoFactory;

    public HttpClient AuthClient => _authFactory!.CreateClient();
    public HttpClient ActivityClient => _activityFactory!.CreateClient();
    public HttpClient GeoClient => _geoFactory!.CreateClient();

    public async Task InitializeAsync()
    {
        await Task.WhenAll(_postgres.StartAsync(), _nats.StartAsync());

        var baseConn = _postgres.GetConnectionString();
        var authConn = RepoLayout.WithDatabase(baseConn, "auth");
        var activityConn = RepoLayout.WithDatabase(baseConn, "activity");
        var geoConn = RepoLayout.WithDatabase(baseConn, "geo");

        var natsUrl = TurboTestContainers.NatsUrl(_nats);
        _authFactory = BuildFactory<Turbo.Host.Auth.AuthHostProgram>(authConn, natsUrl, "Auth");
        _activityFactory = BuildFactory<Turbo.Host.Activity.ActivityHostProgram>(activityConn, natsUrl, "Activity");
        _geoFactory = BuildFactory<Turbo.Host.Geo.GeoHostProgram>(geoConn, natsUrl, "Geo");

        // EF Core's MigrateAsync needs the target DB to exist; the helper
        // creates it lazily. Each host's Program.cs migrates at startup
        // already, but WebApplicationFactory doesn't execute that startup
        // path, so we run the migrations explicitly here.
        await _authFactory.Services.MigrateModuleDatabaseAsync<AuthDbContext>(authConn);
        await _activityFactory.Services.MigrateModuleDatabaseAsync<ActivityContext>(activityConn);
        await _geoFactory.Services.MigrateModuleDatabaseAsync<LocationReadContext>(geoConn);
    }

    public async Task DisposeAsync()
    {
        _authFactory?.Dispose();
        _activityFactory?.Dispose();
        _geoFactory?.Dispose();
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
