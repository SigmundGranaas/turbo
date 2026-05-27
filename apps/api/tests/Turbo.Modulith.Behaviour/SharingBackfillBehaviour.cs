using System.Net.Http.Json;
using FluentAssertions;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Npgsql;
using Turbo.Behaviour.Testing;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.integration;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Covers the one-shot startup backfill that ensures pre-existing
/// collection / marker / path rows get a corresponding Resource
/// envelope in the Sharing schema.
///
/// The sidecar event handlers cover entities created via the
/// /api/collections, /api/geo, /api/tracks endpoints — the backfill
/// exists for the bootstrap case where rows pre-date the Sharing
/// service. Tests verify that path by inserting rows directly into
/// the payload read tables (bypassing the create endpoints, which
/// would fire the sidecar) and then running the backfill.
/// </summary>
[Collection("ModulithHost")]
public sealed class SharingBackfillBehaviour
{
    private readonly ModulithHostFixture _host;
    public SharingBackfillBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task backfill_creates_resource_envelopes_for_pre_existing_rows()
    {
        var owner = Guid.NewGuid();
        var collectionId = Guid.NewGuid();
        var markerId = Guid.NewGuid();
        var trackId = Guid.NewGuid();

        var config = _host.Services.GetRequiredService<IConfiguration>();
        await InsertCollectionDirectly(config.GetConnectionString("Collections")!, collectionId, owner);
        await InsertLocationDirectly(config.GetConnectionString("Geo")!, markerId, owner);
        await InsertTrackDirectly(config.GetConnectionString("Tracks")!, trackId, owner);

        await _host.Services.BackfillSharingResourcesAsync(config);

        var ownerClient = _host.CreateClientAs(owner);
        var page = await ownerClient.GetFromJsonAsync<ResourceSyncPage>(
            "/api/sharing/resources/sync");
        page!.Items.Should().Contain(e => e.Id == collectionId && e.Type == "collection");
        page.Items.Should().Contain(e => e.Id == markerId && e.Type == "marker");
        page.Items.Should().Contain(e => e.Id == trackId && e.Type == "path");
    }

    [Fact]
    public async Task backfill_is_idempotent()
    {
        var owner = Guid.NewGuid();
        var collectionId = Guid.NewGuid();
        var config = _host.Services.GetRequiredService<IConfiguration>();
        await InsertCollectionDirectly(config.GetConnectionString("Collections")!, collectionId, owner);

        await _host.Services.BackfillSharingResourcesAsync(config);
        await _host.Services.BackfillSharingResourcesAsync(config); // second run

        var client = _host.CreateClientAs(owner);
        var page = await client.GetFromJsonAsync<ResourceSyncPage>(
            "/api/sharing/resources/sync");
        page!.Items.Count(e => e.Id == collectionId).Should().Be(1);
    }

    private static async Task InsertCollectionDirectly(string conn, Guid id, Guid ownerId)
    {
        await using var c = new NpgsqlConnection(conn);
        await c.OpenAsync();
        await using var cmd = new NpgsqlCommand(@"
            INSERT INTO collections_read
                (id, owner_id, name, sort_order, created_at, updated_at, version)
            VALUES
                (@id, @owner, 'pre-existing', 0, NOW(), NOW(), 1)",
            c);
        cmd.Parameters.AddWithValue("@id", id);
        cmd.Parameters.AddWithValue("@owner", ownerId);
        await cmd.ExecuteNonQueryAsync();
    }

    private static async Task InsertLocationDirectly(string conn, Guid id, Guid ownerId)
    {
        await using var c = new NpgsqlConnection(conn);
        await c.OpenAsync();
        await using var cmd = new NpgsqlCommand(@"
            INSERT INTO locations_read
                (id, owner_id, name, description, icon, geometry,
                 created_at, updated_at, version)
            VALUES
                (@id, @owner, 'pre-existing', '', 'pin',
                 ST_GeomFromText('POINT(0 0)', 4326),
                 NOW(), NOW(), 1)",
            c);
        cmd.Parameters.AddWithValue("@id", id);
        cmd.Parameters.AddWithValue("@owner", ownerId);
        await cmd.ExecuteNonQueryAsync();
    }

    private static async Task InsertTrackDirectly(string conn, Guid id, Guid ownerId)
    {
        await using var c = new NpgsqlConnection(conn);
        await c.OpenAsync();
        await using var cmd = new NpgsqlCommand(@"
            INSERT INTO tracks_read
                (id, owner_id, name, geometry, distance_meters,
                 created_at, updated_at, version, smoothing)
            VALUES
                (@id, @owner, 'pre-existing',
                 ST_GeomFromText('LINESTRING(0 0, 1 1)', 4326),
                 0, NOW(), NOW(), 1, false)",
            c);
        cmd.Parameters.AddWithValue("@id", id);
        cmd.Parameters.AddWithValue("@owner", ownerId);
        await cmd.ExecuteNonQueryAsync();
    }
}
