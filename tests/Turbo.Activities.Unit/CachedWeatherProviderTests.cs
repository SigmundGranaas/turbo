using FluentAssertions;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using Turboapi.Activities.conditions;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;
using Xunit;

namespace Turbo.Activities.Unit;

/// <summary>
/// Decorator-only tests for CachedWeatherProvider. Wraps an inner
/// provider that records calls so we can assert hit/miss behavior
/// without spinning up Postgres. Same shape covers the other typed
/// decorators (Avalanche/RiverFlow/Tide/Grooming) — they all delegate
/// to the same IConditionsCache contract.
/// </summary>
public sealed class CachedWeatherProviderTests
{
    [Fact]
    public async Task First_call_hits_inner_provider_and_caches()
    {
        var inner = new RecordingWeatherProvider();
        var cache = new InMemoryConditionsCache();
        var clock = new FixedTimeProvider(new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero));
        var sut = new CachedWeatherProvider(inner, cache, NullLogger<CachedWeatherProvider>.Instance, clock);

        var at = new DateTimeOffset(2026, 5, 20, 12, 30, 0, TimeSpan.Zero);
        var result = await sut.GetAsync(60.12, 5.32, at, CancellationToken.None);

        inner.CallCount.Should().Be(1);
        result.AirTemperatureCelsius.Should().Be(10.0f);
        cache.Entries.Should().HaveCount(1);
    }

    [Fact]
    public async Task Second_call_in_same_grid_and_hour_serves_from_cache()
    {
        var inner = new RecordingWeatherProvider();
        var cache = new InMemoryConditionsCache();
        var clock = new FixedTimeProvider(new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero));
        var sut = new CachedWeatherProvider(inner, cache, NullLogger<CachedWeatherProvider>.Instance, clock);

        var at = new DateTimeOffset(2026, 5, 20, 12, 30, 0, TimeSpan.Zero);
        _ = await sut.GetAsync(60.12, 5.32, at, CancellationToken.None);
        var second = await sut.GetAsync(60.121, 5.319, at.AddMinutes(5), CancellationToken.None);

        inner.CallCount.Should().Be(1);
        second.AirTemperatureCelsius.Should().Be(10.0f);
    }

    [Fact]
    public async Task Expired_entry_triggers_refetch()
    {
        var inner = new RecordingWeatherProvider();
        var cache = new InMemoryConditionsCache();
        var clock = new FixedTimeProvider(new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero));
        var sut = new CachedWeatherProvider(inner, cache, NullLogger<CachedWeatherProvider>.Instance, clock);

        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        _ = await sut.GetAsync(60.12, 5.32, at, CancellationToken.None);

        // Advance the wall clock past the 30-minute TTL.
        clock.Now = clock.Now.AddMinutes(45);
        _ = await sut.GetAsync(60.12, 5.32, at, CancellationToken.None);

        inner.CallCount.Should().Be(2);
    }

    [Fact]
    public async Task Different_hour_buckets_each_hit_inner()
    {
        var inner = new RecordingWeatherProvider();
        var cache = new InMemoryConditionsCache();
        var clock = new FixedTimeProvider(new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero));
        var sut = new CachedWeatherProvider(inner, cache, NullLogger<CachedWeatherProvider>.Instance, clock);

        _ = await sut.GetAsync(60.12, 5.32, new DateTimeOffset(2026, 5, 20, 12, 10, 0, TimeSpan.Zero), CancellationToken.None);
        _ = await sut.GetAsync(60.12, 5.32, new DateTimeOffset(2026, 5, 20, 13, 10, 0, TimeSpan.Zero), CancellationToken.None);

        inner.CallCount.Should().Be(2);
    }
}

internal sealed class RecordingWeatherProvider : IWeatherProvider
{
    public string Key => "synthetic_weather";
    public int CallCount { get; private set; }

    public Task<WeatherSlice> GetAsync(double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        CallCount++;
        return Task.FromResult(new WeatherSlice(
            validAt: at,
            airTemperatureCelsius: 10.0f, airPressureHpa: 1013.25f,
            relativeHumidityPct: 70.0f, cloudCoveragePct: 40.0f,
            windSpeedMs: 5.0f, windGustMs: 8.0f, windFromDegrees: 180.0f,
            precipitationNext1hMm: 0.0f, precipitationNext6hMm: 0.0f,
            symbolCode: "partlycloudy_day"));
    }
}

internal sealed class InMemoryConditionsCache : IConditionsCache
{
    public Dictionary<string, CachedConditionsSlice> Entries { get; } = new();

    public Task<CachedConditionsSlice?> TryGetAsync(
        string providerKey, string gridCell, DateTimeOffset timeBucket, CancellationToken cancellationToken)
    {
        var k = Key(providerKey, gridCell, timeBucket);
        return Task.FromResult(Entries.TryGetValue(k, out var v) ? v : null);
    }

    public Task PutAsync(
        string providerKey, string gridCell, DateTimeOffset timeBucket,
        ReadOnlyMemory<byte> payload, DateTimeOffset fetchedAt, DateTimeOffset expiresAt,
        CancellationToken cancellationToken)
    {
        var k = Key(providerKey, gridCell, timeBucket);
        Entries[k] = new CachedConditionsSlice(providerKey, gridCell, timeBucket, payload, fetchedAt, expiresAt);
        return Task.CompletedTask;
    }

    private static string Key(string provider, string grid, DateTimeOffset bucket) =>
        $"{provider}|{grid}|{bucket:O}";
}

internal sealed class FixedTimeProvider : TimeProvider
{
    public DateTimeOffset Now { get; set; }
    public FixedTimeProvider(DateTimeOffset now) { Now = now; }
    public override DateTimeOffset GetUtcNow() => Now;
}
