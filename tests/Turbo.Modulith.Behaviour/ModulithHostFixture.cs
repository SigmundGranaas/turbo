using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Npgsql;
using Testcontainers.PostgreSql;
using Turbo.Behaviour.Testing;
using Turbo.Host.Modulith;
using Turbo.Hosting.Postgres;
using Turboapi.Auth.Infrastructure.Persistence;
using Turboapi.Collections.data;
using Turboapi.Geo.domain.query.model;
using Turboapi.Tracks.data;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Boots <c>Turbo.Host.Modulith</c> on one Postgres container with three
/// separate databases (auth/geo/tracks) — the modulith deploy topology.
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

        var authConn = RepoLayout.WithDatabase(baseConn, "auth");
        var tracksConn = RepoLayout.WithDatabase(baseConn, "tracks");
        var geoConn = RepoLayout.WithDatabase(baseConn, "geo");
        var collectionsConn = RepoLayout.WithDatabase(baseConn, "collections");

        _factory = new WebApplicationFactory<ModulithProgram>().WithWebHostBuilder(builder =>
        {
            builder.UseEnvironment("Test");
            builder.UseSetting("ConnectionStrings:Auth", authConn);
            builder.UseSetting("ConnectionStrings:Tracks", tracksConn);
            builder.UseSetting("ConnectionStrings:Geo", geoConn);
            builder.UseSetting("ConnectionStrings:Collections", collectionsConn);
        });

        await _factory.Services.MigrateModuleDatabaseAsync<AuthDbContext>(authConn);
        await _factory.Services.MigrateModuleDatabaseAsync<TrackReadContext>(tracksConn);
        await _factory.Services.MigrateModuleDatabaseAsync<LocationReadContext>(geoConn);
        await _factory.Services.MigrateModuleDatabaseAsync<CollectionsReadContext>(collectionsConn);
    }

    public async Task DisposeAsync()
    {
        _factory?.Dispose();
        await _postgres.DisposeAsync();
    }

    public Turbo.Messaging.InProcess.InProcessMessageBus Bus
        => _factory!.Services.GetRequiredService<Turbo.Messaging.InProcess.InProcessMessageBus>();

    /// <summary>
    /// Reads the most-recent tracks-outbox row whose event type ends
    /// with <paramref name="eventTypeSuffix"/> and republishes it on the
    /// in-process bus as if the broker had redelivered.
    /// </summary>
    public async Task RedeliverLatestTracksEnvelopeAsync(string eventTypeSuffix)
    {
        var tracksConn = RepoLayout.WithDatabase(_postgres.GetConnectionString(), "tracks");
        await using var conn = new NpgsqlConnection(tracksConn);
        await conn.OpenAsync();

        await using var cmd = new NpgsqlCommand(
            @"SELECT id, event_type, source, data_content_type, payload_json, headers_json, occurred_at
              FROM tracks.outbox
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
    /// Counts non-tombstoned rows in the tracks read model matching the id.
    /// </summary>
    public async Task<int> CountTracksRowsAsync(Guid trackId)
    {
        var tracksConn = RepoLayout.WithDatabase(_postgres.GetConnectionString(), "tracks");
        await using var conn = new NpgsqlConnection(tracksConn);
        await conn.OpenAsync();
        await using var cmd = new NpgsqlCommand(
            "SELECT COUNT(*) FROM tracks_read WHERE id = @id AND deleted_at IS NULL;", conn);
        cmd.Parameters.AddWithValue("@id", trackId);
        var result = await cmd.ExecuteScalarAsync();
        return Convert.ToInt32(result);
    }

    /// <summary>
    /// Test-only simulation of the operator rebuild SOP: truncate the
    /// tracks read model + the tracks dedup table, then mark every
    /// tracks outbox row as undispatched.
    /// </summary>
    public async Task ResetTracksReadModelForReplayAsync()
    {
        var tracksConn = RepoLayout.WithDatabase(_postgres.GetConnectionString(), "tracks");
        await using var conn = new NpgsqlConnection(tracksConn);
        await conn.OpenAsync();

        await using var truncateReadModel = new NpgsqlCommand("TRUNCATE TABLE tracks_read;", conn);
        await truncateReadModel.ExecuteNonQueryAsync();

        await using var truncateDedup = new NpgsqlCommand("TRUNCATE TABLE tracks.processed_events;", conn);
        await truncateDedup.ExecuteNonQueryAsync();

        await using var resetOutbox = new NpgsqlCommand(
            "UPDATE tracks.outbox SET dispatched_at = NULL, attempts = 0, last_error = NULL;", conn);
        await resetOutbox.ExecuteNonQueryAsync();
    }
}

[CollectionDefinition("ModulithHost")]
public sealed class ModulithHostCollection : ICollectionFixture<ModulithHostFixture> { }
