using System.Net.Http.Headers;
using DotNet.Testcontainers.Containers;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Testcontainers.PostgreSql;
using Xunit;

namespace Turbo.Behaviour.Testing;

/// <summary>
/// Common Postgres + NATS + WebApplicationFactory wiring for a single-module
/// host. Subclass per host (Auth/Activity/Geo) and override
/// <see cref="ConfigureTestServices"/> + <see cref="MigrateAsync"/>.
/// </summary>
public abstract class TurboHostFixture<THost> : IAsyncLifetime where THost : class
{
    private readonly PostgreSqlContainer _postgres;
    private readonly IContainer _nats = TurboTestContainers.NatsJetStream();
    private WebApplicationFactory<THost>? _factory;
    private TurboJwtIssuer? _jwt;

    protected TurboHostFixture(string databaseName)
    {
        _postgres = TurboTestContainers.PostgresWithPostGis(databaseName);
    }

    /// <summary>
    /// Hook for replacing the module's DbContext registration with one that
    /// points at the Testcontainers Postgres. Use <see cref="ReplaceDbContext"/>.
    /// </summary>
    protected abstract void ConfigureTestServices(WebHostBuilderContext context, IServiceCollection services);

    /// <summary>
    /// The <c>ConnectionStrings:&lt;Key&gt;</c> name the host reads at
    /// startup (e.g. "Auth", "Geo", "Activity"). The fixture overrides this
    /// configuration entry to point at the Testcontainers Postgres so the
    /// host's <c>MigrateModuleDatabaseAsync</c> call at <c>Program.cs</c>
    /// uses the test database.
    /// </summary>
    protected abstract string ConnectionStringKey { get; }

    /// <summary>
    /// Subclass hook for extra config overrides — e.g. wiring
    /// <c>ConnectionStrings:Sharing</c> on a payload-module host that
    /// consults IAccessControl. Override to return a populated map;
    /// defaults to empty.
    /// </summary>
    protected virtual IDictionary<string, string?> ExtraSettings
        => new Dictionary<string, string?>();

    /// <summary>
    /// Runs the module's EF Core migrations against the Testcontainers
    /// Postgres after the host's <see cref="WebApplicationFactory{THost}"/>
    /// has built its service provider. Typical implementation:
    /// <c>await services.MigrateModuleDatabaseAsync&lt;MyDbContext&gt;(ConnectionString)</c>.
    /// </summary>
    protected abstract Task MigrateAsync(IServiceProvider services);

    protected string ConnectionString => _postgres.GetConnectionString();

    public HttpClient CreateClient() => _factory!.CreateClient();

    public HttpClient CreateClientAs(Guid userId)
    {
        var client = _factory!.CreateClient();
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", _jwt!.Issue(userId));
        return client;
    }

    public async Task InitializeAsync()
    {
        await Task.WhenAll(_postgres.StartAsync(), _nats.StartAsync());

        _factory = new WebApplicationFactory<THost>().WithWebHostBuilder(builder =>
        {
            builder.UseEnvironment("Test");
            builder.UseContentRoot(RepoLayout.HostContentRoot<THost>());
            builder.UseSetting("Nats:Url", TurboTestContainers.NatsUrl(_nats));
            // Override the connection string the host's Program.cs reads
            // when it calls MigrateModuleDatabaseAsync at startup.
            builder.UseSetting($"ConnectionStrings:{ConnectionStringKey}", _postgres.GetConnectionString());
            // Subclass-supplied overrides — e.g. payload-module hosts that
            // need ConnectionStrings:Sharing wired for their IAccessControl.
            foreach (var kv in ExtraSettings)
            {
                builder.UseSetting(kv.Key, kv.Value);
            }
            builder.ConfigureServices((context, services) =>
            {
                _jwt = new TurboJwtIssuer(context.Configuration["Jwt:Key"]
                    ?? throw new InvalidOperationException("Jwt:Key not configured for Test environment"));
                ConfigureTestServices(context, services);
            });
        });

        // Force the host to build its service provider; then migrate. Tests
        // run against the same provider the request pipeline sees, so the
        // DbContext registration we just swapped in is the one MigrateAsync
        // operates on.
        await MigrateAsync(_factory.Services);
    }

    public async Task DisposeAsync()
    {
        _factory?.Dispose();
        await Task.WhenAll(_nats.DisposeAsync().AsTask(), _postgres.DisposeAsync().AsTask());
    }

    public Task PauseBrokerAsync() => _nats.PauseAsync();
    public Task UnpauseBrokerAsync() => _nats.UnpauseAsync();

    /// <summary>
    /// Swap a DbContext's options registration for one bound to this fixture's
    /// Testcontainers Postgres. Use from <see cref="ConfigureTestServices"/>.
    /// </summary>
    protected static void ReplaceDbContext<TContext>(
        IServiceCollection services,
        Action<DbContextOptionsBuilder> configure) where TContext : DbContext
    {
        var dbDescriptor = services.SingleOrDefault(d =>
            d.ServiceType == typeof(DbContextOptions<TContext>));
        if (dbDescriptor is not null) services.Remove(dbDescriptor);
        services.AddDbContext<TContext>(configure);
    }
}
