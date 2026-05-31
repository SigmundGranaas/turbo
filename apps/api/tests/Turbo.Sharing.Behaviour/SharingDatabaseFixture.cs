using Microsoft.EntityFrameworkCore;
using Testcontainers.PostgreSql;
using Turbo.Behaviour.Testing;
using Turboapi.Sharing.data;
using Xunit;

namespace Turbo.Sharing.Behaviour;

/// <summary>
/// Spins up a Postgres container and applies the SharingReadContext
/// migrations once. Tests open a fresh DbContext per call via
/// <see cref="CreateContext"/>; the database is shared across tests in
/// the same xUnit collection but is wiped between tests by
/// <see cref="ResetAsync"/>.
/// </summary>
public sealed class SharingDatabaseFixture : IAsyncLifetime
{
    private readonly PostgreSqlContainer _postgres
        = TurboTestContainers.PostgresWithPostGis("sharing");

    private DbContextOptions<SharingReadContext>? _options;

    public string ConnectionString => _postgres.GetConnectionString();

    public async Task InitializeAsync()
    {
        await _postgres.StartAsync();
        _options = new DbContextOptionsBuilder<SharingReadContext>()
            .UseNpgsql(ConnectionString)
            .Options;

        await using var ctx = new SharingReadContext(_options);
        await ctx.Database.MigrateAsync();
    }

    public Task DisposeAsync() => _postgres.DisposeAsync().AsTask();

    public SharingReadContext CreateContext()
        => new(_options ?? throw new InvalidOperationException("Fixture not initialized"));

    /// <summary>Wipes all rows but keeps the schema, fast between tests.</summary>
    public async Task ResetAsync()
    {
        await using var ctx = CreateContext();
        await ctx.Database.ExecuteSqlRawAsync(
            "TRUNCATE sharing.resources, sharing.grants, sharing.friendships, " +
            "sharing.groups, sharing.group_members, sharing.share_invites RESTART IDENTITY CASCADE");
    }
}

[CollectionDefinition("SharingDatabase")]
public sealed class SharingDatabaseCollection : ICollectionFixture<SharingDatabaseFixture> { }
