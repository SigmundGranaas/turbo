using System.Collections.Concurrent;
using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using FluentAssertions;
using Turboapi.Places;
using Turboapi.Places.Infrastructure;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P1b durability gate: an atomic version swap under concurrent read load.
/// Readers must keep serving — zero failed requests — and every response must
/// be a complete version (never a half-swapped mix), with the ETag flipping
/// exactly once. Own host instance (its own container) since the swap wipes
/// the seeded data.
/// </summary>
public class PlacesSwapUnderLoadBehaviour : IClassFixture<PlacesHostFixture>
{
    private const string Url = "/api/places/reverse?lat=61.6363&lon=8.3120";
    private const string SeededTitle = "Galdhøpiggen";
    private const string SwappedTitle = "Svingfjellet";

    private readonly PlacesHostFixture _fixture;

    public PlacesSwapUnderLoadBehaviour(PlacesHostFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Atomic_swap_under_concurrent_reads_never_fails_and_flips_once()
    {
        var client = _fixture.CreateClient();

        // Baseline: the seeded data answers Galdhøpiggen.
        var warm = await client.GetFromJsonAsync<JsonElement>(Url);
        warm.GetProperty("title").GetString().Should().Be(SeededTitle);

        // Stage a v2 whose only feature sits exactly on the query point.
        var store = new PgPlaceStore(_fixture.ConnectionString);
        await store.StagePlacesAsync(
            [new Place("test", "swap-peak", "Fjelltopp", SwappedTitle, 61.6363, 8.3120, "aktiv")],
            "swap-load-v2");

        var failures = new ConcurrentBag<string>();
        var titles = new ConcurrentBag<string>();
        using var cts = new CancellationTokenSource();

        var workers = Enumerable.Range(0, 8).Select(_ => Task.Run(async () =>
        {
            while (!cts.IsCancellationRequested)
            {
                try
                {
                    var resp = await client.GetAsync(Url);
                    if (resp.StatusCode != HttpStatusCode.OK)
                    {
                        failures.Add($"status {(int)resp.StatusCode}");
                        continue;
                    }
                    var d = await resp.Content.ReadFromJsonAsync<JsonElement>();
                    titles.Add(d.GetProperty("title").GetString()!);
                }
                catch (Exception ex)
                {
                    failures.Add(ex.Message);
                }
            }
        })).ToArray();

        await Task.Delay(150);            // let reads ramp under the old version
        await store.SwapAsync("swap-load-v2");
        await Task.Delay(300);            // keep reading across + after the swap
        cts.Cancel();
        await Task.WhenAll(workers);

        failures.Should().BeEmpty("no request may fail during an atomic swap");
        titles.Should().NotBeEmpty();
        titles.Should().OnlyContain(t => t == SeededTitle || t == SwappedTitle,
            "every response is a complete version — never a half-swapped mix");
        titles.Should().Contain(SwappedTitle, "the new version must become visible");

        // Settled state: new title, ETag flipped to the new version exactly.
        var after = await client.GetAsync(Url);
        after.Headers.ETag!.Tag.Should().Be("\"swap-load-v2\"");
        var final = await after.Content.ReadFromJsonAsync<JsonElement>();
        final.GetProperty("title").GetString().Should().Be(SwappedTitle);
    }
}
