using NetTopologySuite.Geometries;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Source of a single piece of environmental data (weather, tides,
/// avalanche, river flow, …) for a location at a time. Each kind composes
/// the providers it needs via its own <c>IConditionsAdvisor</c> rather than
/// inheriting from a common conditions base. Implementations are expected
/// to consult a server-side cache before hitting upstream APIs and to
/// respect each upstream's TOS (User-Agent header, rate limits, attribution).
/// </summary>
public interface IConditionsProvider
{
    string Key { get; }

    bool SupportsGeometry(Geometry geometry);

    Task<ConditionsSlice> GetAsync(Geometry geometry, DateTimeOffset at, CancellationToken cancellationToken);
}

/// <summary>
/// Opaque-by-design wrapper around a provider's typed payload. Each
/// kind-specific advisor knows how to project its own slice subset into a
/// typed report.
/// </summary>
public sealed record ConditionsSlice(string ProviderKey, DateTimeOffset FetchedAt, DateTimeOffset ValidAt, object Payload);
