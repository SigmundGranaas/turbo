using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Npgsql;
using Testcontainers.PostgreSql;
using Turbo.Behaviour.Testing;
using Turbo.Host.Modulith;
using Turbo.Hosting.Postgres;
using Turboapi.Activity.data;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Geo.domain.query.model;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Boots <c>Turbo.Host.Modulith</c> on one Postgres container with three
/// separate databases (auth/activity/geo) — the modulith deploy topology.
/// No NATS; the in-process transport delivers events.
/// </summary>
public sealed class ModulithHostFixture : IAsyncLifetime
{
    private readonly PostgreSqlContainer _postgres = TurboTestContainers.PostgresWithPostGis();

    private WebApplicationFactory<ModulithProgram>? _factory;

    public HttpClient CreateClient() => _factory!.CreateClient();

    public async Task InitializeAsync()
    {
        await _postgres.StartAsync();
        var baseConn = _postgres.GetConnectionString();

        // Modulith hosts three databases on one Postgres. EF Core's
        // MigrateAsync creates schema but not the database itself; the host
        // helper handles both at startup once the factory is up. The
        // databases themselves are created lazily by MigrateModuleDatabaseAsync.
        var authConn = RepoLayout.WithDatabase(baseConn, "auth");
        var activityConn = RepoLayout.WithDatabase(baseConn, "activity");
        var geoConn = RepoLayout.WithDatabase(baseConn, "geo");

        _factory = new WebApplicationFactory<ModulithProgram>().WithWebHostBuilder(builder =>
        {
            builder.UseEnvironment("Test");
            builder.UseSetting("ConnectionStrings:Auth", authConn);
            builder.UseSetting("ConnectionStrings:Activity", activityConn);
            builder.UseSetting("ConnectionStrings:Geo", geoConn);
        });

        // The test factory doesn't execute Program.cs's top-level await, so
        // the migrations have to run here against the test-overridden DbContexts.
        await _factory.Services.MigrateModuleDatabaseAsync<AuthDbContext>(authConn);
        await _factory.Services.MigrateModuleDatabaseAsync<ActivityContext>(activityConn);
        await _factory.Services.MigrateModuleDatabaseAsync<LocationReadContext>(geoConn);
    }

    public async Task DisposeAsync()
    {
        _factory?.Dispose();
        await _postgres.DisposeAsync();
    }

    /// <summary>
    /// Resolves the running modulith host's in-process bus so a test can
    /// simulate at-least-once redelivery — publishing the same envelope
    /// twice to assert the idempotency table dedupes.
    /// </summary>
    public Turbo.Messaging.InProcess.InProcessMessageBus Bus
        => _factory!.Services.GetRequiredService<Turbo.Messaging.InProcess.InProcessMessageBus>();

    /// <summary>
    /// Reads the most-recent activity-outbox row whose event type ends
    /// with <paramref name="eventTypeSuffix"/> and republishes it on the
    /// in-process bus as if the broker had redelivered.
    /// </summary>
    public async Task RedeliverLatestActivityEnvelopeAsync(string eventTypeSuffix)
    {
        var activityConn = RepoLayout.WithDatabase(_postgres.GetConnectionString(), "activity");
        await using var conn = new NpgsqlConnection(activityConn);
        await conn.OpenAsync();

        await using var cmd = new NpgsqlCommand(
            @"SELECT id, event_type, source, data_content_type, payload_json, headers_json, occurred_at
              FROM activity.outbox
              WHERE event_type LIKE @suffix
              ORDER BY position DESC
              LIMIT 1;", conn);
        cmd.Parameters.AddWithValue("@suffix", "%" + eventTypeSuffix);
        await using var reader = await cmd.ExecuteReaderAsync();
        if (!await reader.ReadAsync())
            throw new InvalidOperationException($"No outbox row found matching event_type LIKE %{eventTypeSuffix}");

        var envelope = new Turbo.Messaging.EventEnvelope(
            EventId: reader.GetGuid(0),
            Type: reader.GetString(1),
            Source: reader.GetString(2),
            Time: reader.GetDateTime(6),
            DataContentType: reader.GetString(3),
            Data: System.Text.Encoding.UTF8.GetBytes(reader.GetString(4)),
            Headers: System.Text.Json.JsonSerializer.Deserialize<Dictionary<string, string>>(reader.GetString(5))
                     ?? new Dictionary<string, string>());

        await reader.CloseAsync();
        await Bus.PublishAsync(envelope, CancellationToken.None);
    }

    /// <summary>
    /// Counts rows in the activity read model. Used by the idempotency
    /// test as an authoritative check that a redelivery did not insert
    /// a duplicate row.
    /// </summary>
    public async Task<int> CountActivityRowsAsync(Guid activityId)
    {
        var activityConn = RepoLayout.WithDatabase(_postgres.GetConnectionString(), "activity");
        await using var conn = new NpgsqlConnection(activityConn);
        await conn.OpenAsync();
        await using var cmd = new NpgsqlCommand(
            "SELECT COUNT(*) FROM activity_query WHERE activity_id = @id;", conn);
        cmd.Parameters.AddWithValue("@id", activityId);
        var result = await cmd.ExecuteScalarAsync();
        return Convert.ToInt32(result);
    }

    /// <summary>
    /// Test-only simulation of the operator rebuild SOP: truncate the
    /// activity read model + the activity dedup table, then mark every
    /// activity outbox row as undispatched.
    /// </summary>
    public async Task ResetActivityReadModelForReplayAsync()
    {
        var activityConn = RepoLayout.WithDatabase(_postgres.GetConnectionString(), "activity");
        await using var conn = new NpgsqlConnection(activityConn);
        await conn.OpenAsync();

        await using var truncateReadModel = new NpgsqlCommand("TRUNCATE TABLE activity_query;", conn);
        await truncateReadModel.ExecuteNonQueryAsync();

        await using var truncateDedup = new NpgsqlCommand("TRUNCATE TABLE activity.processed_events;", conn);
        await truncateDedup.ExecuteNonQueryAsync();

        await using var resetOutbox = new NpgsqlCommand(
            "UPDATE activity.outbox SET dispatched_at = NULL, attempts = 0, last_error = NULL;", conn);
        await resetOutbox.ExecuteNonQueryAsync();
    }
}

[CollectionDefinition("ModulithHost")]
public sealed class ModulithHostCollection : ICollectionFixture<ModulithHostFixture> { }
