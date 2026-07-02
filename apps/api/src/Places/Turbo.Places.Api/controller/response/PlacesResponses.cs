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

public sealed record PlacesHealthResponse(long Places, long Areas, string? DatasetVersion, string Attribution);

/// <summary>One ingest-run ledger entry for the ops surface.</summary>
public sealed record IngestRunResponse(
    string Source,
    string Status,
    DateTimeOffset StartedAt,
    DateTimeOffset? FinishedAt,
    string? SourceVersion,
    long RowsWritten,
    string? Error);

public sealed record IngestRunsResponse(IReadOnlyList<IngestRunResponse> Runs);

public sealed record ErrorResponse(string Code, string Detail);
