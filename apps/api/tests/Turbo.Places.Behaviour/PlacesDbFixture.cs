using Testcontainers.PostgreSql;
using Turboapi.Places.Infrastructure;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// PostGIS-only fixture for store-level tests (staging/swap/sweep) — no host,
/// no place-core .so. Lighter than <see cref="PlacesHostFixture"/>.
/// </summary>
public sealed class PlacesDbFixture : IAsyncLifetime
{
    private readonly PostgreSqlContainer _postgres = new PostgreSqlBuilder()
        .WithImage("postgis/postgis:16-3.4")
        .WithDatabase("places")
        .Build();

    public string ConnectionString => _postgres.GetConnectionString();
    public PgPlaceStore Store => new(ConnectionString);

    public async Task InitializeAsync()
    {
        await _postgres.StartAsync();
        await Store.EnsureSchemaAsync();
    }

    public Task DisposeAsync() => _postgres.DisposeAsync().AsTask();
}
