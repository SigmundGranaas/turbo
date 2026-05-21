using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Live grooming status for an xc-ski trail from an external feed
/// (e.g. skisporet.no). <c>HoursAgo</c> is computed by the provider
/// from the upstream timestamp; the xc-ski advisor uses it to derive
/// a fresher score than the activity's stored
/// <c>GroomingStatus</c> enum.
/// </summary>
public sealed record GroomingSlice
{
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("hoursAgo")] public int HoursAgo { get; init; }
    [JsonPropertyName("summary")] public string? Summary { get; init; }

    [JsonConstructor]
    public GroomingSlice(DateTimeOffset validAt, int hoursAgo, string? summary)
    {
        ValidAt = validAt;
        HoursAgo = hoursAgo;
        Summary = summary;
    }
}

/// <summary>
/// Grooming-feed source for an xc-ski trail. Implementations live in
/// Shared.Infrastructure.
/// </summary>
public interface IGroomingProvider
{
    string Key { get; }

    /// <summary>Look up the most recent grooming pass for the trail
    /// referenced by <paramref name="feedKey"/>. The feed key is an
    /// opaque string the kind module stores per activity (e.g. the
    /// skisporet.no trail id).</summary>
    Task<GroomingSlice> GetAsync(
        string feedKey,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
