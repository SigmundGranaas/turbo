using FluentAssertions;
using Npgsql;
using Turboapi.Places;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P1b: versioned ingestion — stage → atomic swap, sweeping features absent in
/// the new version, and resumable after a crash before swap. The swap-under-load
/// durability test lives in <see cref="PlacesSwapUnderLoadBehaviour"/>.
/// </summary>
public class DatasetSwapTests : IClassFixture<PlacesDbFixture>
{
    private readonly PlacesDbFixture _fixture;

    public DatasetSwapTests(PlacesDbFixture fixture) => _fixture = fixture;

    private static Place Peak(string id, string name, double lat, double lng) =>
        new("ssr", id, "Fjelltopp", name, lat, lng, "aktiv");

    private async Task<string[]> LiveNamesAsync()
    {
        await using var conn = new NpgsqlConnection(_fixture.ConnectionString);
        await conn.OpenAsync();
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = "SELECT primary_name FROM places.places ORDER BY primary_name";
        var names = new List<string>();
        await using var r = await cmd.ExecuteReaderAsync();
        while (await r.ReadAsync()) names.Add(r.GetString(0));
        return names.ToArray();
    }

    [Fact]
    public async Task Swap_replaces_live_and_sweeps_removed_features()
    {
        var store = _fixture.Store;

        await store.StagePlacesAsync([Peak("a", "Alpha", 61.0, 8.0), Peak("b", "Beta", 61.1, 8.1)], "swap-v1");
        await store.SwapAsync("swap-v1");

        (await LiveNamesAsync()).Should().BeEquivalentTo("Alpha", "Beta");
        (await store.GetActiveDatasetVersionAsync()).Should().Be("swap-v1");

        // v2 drops Beta, adds Gamma.
        await store.StagePlacesAsync([Peak("a", "Alpha", 61.0, 8.0), Peak("c", "Gamma", 61.2, 8.2)], "swap-v2");
        await store.SwapAsync("swap-v2");

        (await LiveNamesAsync()).Should().BeEquivalentTo(new[] { "Alpha", "Gamma" },
            "Beta is absent from v2 and must be swept");
        (await store.GetActiveDatasetVersionAsync()).Should().Be("swap-v2");
    }

    [Fact]
    public async Task Crash_before_swap_leaves_live_unchanged_then_resumes_without_duplicates()
    {
        var store = _fixture.Store;

        await store.StagePlacesAsync([Peak("ra", "Resume-A", 60.0, 7.0)], "res-v1");
        await store.SwapAsync("res-v1");

        // "Crash": stage v2 but never swap.
        await store.StagePlacesAsync(
            [Peak("ra", "Resume-A", 60.0, 7.0), Peak("rb", "Resume-B", 60.1, 7.1)], "res-v2");

        (await LiveNamesAsync()).Should().BeEquivalentTo(new[] { "Resume-A" },
            "live stays on v1 until a swap happens");
        (await store.GetActiveDatasetVersionAsync()).Should().Be("res-v1");

        // Resume: re-stage (idempotent) then swap.
        await store.StagePlacesAsync(
            [Peak("ra", "Resume-A", 60.0, 7.0), Peak("rb", "Resume-B", 60.1, 7.1)], "res-v2");
        await store.SwapAsync("res-v2");

        (await LiveNamesAsync()).Should().BeEquivalentTo(new[] { "Resume-A", "Resume-B" },
            "exactly one Resume-A — the swap replaces, it doesn't duplicate");
        (await store.GetActiveDatasetVersionAsync()).Should().Be("res-v2");
    }
}
