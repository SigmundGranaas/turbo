namespace Turboapi.Activities.domain.exception;

/// <summary>
/// Parses HTTP <c>If-Match</c> header values into the row version used
/// by per-kind aggregates. RFC 7232 allows weak / strong ETags wrapped
/// in quotes (eg <c>"42"</c>, <c>W/"42"</c>); our writers always emit
/// strong quoted longs so the parser accepts both forms but only the
/// numeric body matters. Returns null for missing / unparseable
/// headers — callers treat null as "no concurrency precondition".
///
/// Lives in Shared.Core (not Shared.Api) so each kind's controller can
/// use it without taking a dependency on the activities shell. The
/// helper is pure string in/long out — controllers translate
/// <c>StringValues</c> to <c>string?</c> at the boundary.
/// </summary>
public static class IfMatchHeader
{
    public static long? Parse(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return null;
        var span = raw.AsSpan().Trim();
        if (span.StartsWith("W/")) span = span[2..];
        if (span.Length >= 2 && span[0] == '"' && span[^1] == '"') span = span[1..^1];
        return long.TryParse(span, out var v) ? v : null;
    }
}

/// <summary>
/// Body shape for 412 Precondition Failed responses on activity
/// writes. Surfaces both the version the caller sent and the current
/// row version so the client can merge and retry.
/// </summary>
public sealed record ConcurrencyErrorResponse(long ExpectedVersion, long ActualVersion);
