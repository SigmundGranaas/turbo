using System.Text.Json;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Pulls scalar metrics out of cached <see cref="WeatherSlice"/> payloads
/// for percentile queries (e.g. "today's pressure vs the DOY ±7 historical
/// distribution at this grid"). The snapshot store calls this through
/// <see cref="IMetricExtractorRegistry"/>; both the provider key and the
/// metric key (matching the <c>WeatherSlice</c> JSON property name) come
/// from the orchestrator that issued the query.
/// </summary>
public sealed class WeatherMetricExtractor : IMetricExtractor
{
    public WeatherMetricExtractor(string providerKey)
    {
        ProviderKey = providerKey;
    }

    public string ProviderKey { get; }

    public double? Extract(ReadOnlyMemory<byte> payload, string metricKey)
    {
        try
        {
            using var doc = JsonDocument.Parse(payload);
            if (!doc.RootElement.TryGetProperty(metricKey, out var el)) return null;
            return el.ValueKind switch
            {
                JsonValueKind.Number => el.TryGetDouble(out var d) ? d : null,
                JsonValueKind.Null => null,
                _ => null,
            };
        }
        catch (JsonException)
        {
            return null;
        }
    }
}

/// <summary>
/// Pulls <see cref="RiverFlowSlice.CurrentCumecs"/> out of cached river-flow
/// payloads. The fishing + packrafting orchestrators query for
/// "currentCumecs vs DOY percentile at this station".
/// </summary>
public sealed class RiverFlowMetricExtractor : IMetricExtractor
{
    public RiverFlowMetricExtractor(string providerKey)
    {
        ProviderKey = providerKey;
    }

    public string ProviderKey { get; }

    public double? Extract(ReadOnlyMemory<byte> payload, string metricKey)
    {
        try
        {
            using var doc = JsonDocument.Parse(payload);
            if (!doc.RootElement.TryGetProperty(metricKey, out var el)) return null;
            return el.ValueKind == JsonValueKind.Number && el.TryGetDouble(out var d) ? d : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }
}

/// <summary>
/// Same shape, different key — gridded snow's snowDepthCm + sweMm power
/// "SwePctOfNormal" drivers across both ski kinds.
/// </summary>
public sealed class GriddedSnowMetricExtractor : IMetricExtractor
{
    public GriddedSnowMetricExtractor(string providerKey)
    {
        ProviderKey = providerKey;
    }

    public string ProviderKey { get; }

    public double? Extract(ReadOnlyMemory<byte> payload, string metricKey)
    {
        try
        {
            using var doc = JsonDocument.Parse(payload);
            if (!doc.RootElement.TryGetProperty(metricKey, out var el)) return null;
            return el.ValueKind == JsonValueKind.Number && el.TryGetDouble(out var d) ? d : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }
}
