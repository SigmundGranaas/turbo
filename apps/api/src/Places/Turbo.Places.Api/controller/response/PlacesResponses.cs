namespace Turboapi.Places.controller.response;

/// <summary>Maps 1:1 onto the clients' <c>LocationDescription</c>.</summary>
/// <param name="Qualifier"><c>on</c> | <c>closeTo</c> | <c>atPlace</c> |
/// <c>inArea</c> | <c>near</c> | null — the place-core canonical set.</param>
public sealed record ReverseResponse(
    string Title,
    string? Qualifier,
    string? Secondary,
    string? Kommune,
    string? Fylke,
    double? DistanceMeters,
    double? ElevationMeters);

/// <summary>Maps onto the clients' <c>LocationSearchResult</c>.</summary>
public sealed record SearchHitResponse(
    string Title,
    string? Description,
    string Icon,
    double Lat,
    double Lng);

public sealed record SearchResponse(IReadOnlyList<SearchHitResponse> Items);

public sealed record PlacesHealthResponse(long Places, long Areas, string? DatasetVersion);

public sealed record ErrorResponse(string Code, string Detail);
