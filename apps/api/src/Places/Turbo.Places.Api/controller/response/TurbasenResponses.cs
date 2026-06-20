using Turboapi.Places.Core;

namespace Turboapi.Places.controller.response;

/// <summary>The viewport POI list returned to clients. Each <see cref="NtbPoi"/>
/// is already normalised (type/lat/lng/title/summary/imageUrl/utUrl), so the
/// mobile clients deserialize it directly.</summary>
public sealed record NtbPoisResponse(IReadOnlyList<NtbPoi> Pois);
