using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Skisporet (skisporet.no) grooming-feed provider. Fetches the
/// trail's most recent grooming pass and projects it into the typed
/// <see cref="GroomingSlice"/>.
///
/// Skisporet's contract is informal and changes occasionally; this
/// provider talks to the well-known JSON endpoint for a single trail
/// by id. The feed key stored on each xc-ski activity is the
/// skisporet trail id.
///
/// Wiring: registered only when <c>Skisporet:Enabled=true</c>;
/// otherwise <see cref="SyntheticGroomingProvider"/> is wired in its
/// place.
/// </summary>
public sealed class SkisporetGroomingProvider : IGroomingProvider
{
    public const string HttpClientName = "skisporet";

    public string Key => "skisporet_grooming";

    private readonly IHttpClientFactory _http;
    private readonly ILogger<SkisporetGroomingProvider> _logger;

    public SkisporetGroomingProvider(IHttpClientFactory http, ILogger<SkisporetGroomingProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<GroomingSlice> GetAsync(
        string feedKey, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var client = _http.CreateClient(HttpClientName);
        SkisporetTrail? response;
        try
        {
            response = await client.GetFromJsonAsync<SkisporetTrail>($"api/trails/{feedKey}", cancellationToken);
        }
        catch (HttpRequestException ex)
        {
            throw new ConditionsProviderException("Skisporet upstream request failed", ex);
        }
        catch (JsonException ex)
        {
            throw new ConditionsProviderException("Skisporet returned malformed JSON", ex);
        }
        if (response is null)
            throw new ConditionsProviderException("Skisporet returned empty body");
        if (response.LastGrooming is null)
            throw new ConditionsProviderException($"Skisporet has no grooming record for trail {feedKey}");

        var hoursAgo = (int)Math.Max(0, (at - response.LastGrooming.Value).TotalHours);
        var summary = hoursAgo < 12 ? "groomed today" : hoursAgo < 36 ? "groomed yesterday" : $"groomed {hoursAgo / 24}d ago";
        return new GroomingSlice(
            validAt: response.LastGrooming.Value,
            hoursAgo: hoursAgo,
            summary: summary);
    }
}

internal sealed record SkisporetTrail
{
    [JsonPropertyName("lastGrooming")] public DateTimeOffset? LastGrooming { get; init; }
}

public sealed class SkisporetOptions
{
    public bool Enabled { get; set; }
    public string BaseUrl { get; set; } = "https://www.skisporet.no/";
}
