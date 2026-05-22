using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Varsom (api.varsom.no) avalanche-forecast provider. Hits the
/// region-bulletin endpoint for the given region + day, projects the
/// single most-relevant forecast into the typed <see cref="AvalancheSlice"/>.
///
/// Endpoint URL pattern (subject to Varsom API version changes):
///   /api/AvalancheWarningByRegion/Simple/{regionId}/{langKey}/{from}/{to}
/// Today only "simple" forecast is fetched; downstream advisors that
/// need the full per-elevation / per-problem matrix can extend
/// <see cref="VarsomBulletin"/> + project additional fields.
///
/// Wiring: only registered when <c>Varsom:Enabled=true</c>; otherwise
/// <see cref="SyntheticAvalancheProvider"/> is wired in its place.
/// </summary>
public sealed class VarsomAvalancheProvider : IAvalancheProvider
{
    public const string HttpClientName = "varsom";

    public string Key => "varsom_avalanche";

    private readonly IHttpClientFactory _http;
    private readonly ILogger<VarsomAvalancheProvider> _logger;

    public VarsomAvalancheProvider(IHttpClientFactory http, ILogger<VarsomAvalancheProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<AvalancheSlice> GetAsync(
        int varsomRegionId, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var client = _http.CreateClient(HttpClientName);
        var day = at.UtcDateTime.Date;
        var from = day.ToString("yyyy-MM-dd");
        var to = day.AddDays(1).ToString("yyyy-MM-dd");
        var url = $"api/AvalancheWarningByRegion/Simple/{varsomRegionId}/1/{from}/{to}";

        List<VarsomBulletin>? bulletins;
        try
        {
            bulletins = await client.GetFromJsonAsync<List<VarsomBulletin>>(url, cancellationToken);
        }
        catch (HttpRequestException ex)
        {
            throw new ConditionsProviderException("Varsom upstream request failed", ex);
        }
        catch (JsonException ex)
        {
            throw new ConditionsProviderException("Varsom returned malformed JSON", ex);
        }
        if (bulletins is null)
            throw new ConditionsProviderException("Varsom returned empty body");
        if (bulletins.Count == 0)
            throw new ConditionsProviderException(
                $"Varsom returned no bulletins for region {varsomRegionId} on {from}");

        var b = bulletins[0];
        return new AvalancheSlice(
            validFor: new DateTimeOffset(day, TimeSpan.Zero),
            dangerLevel: int.TryParse(b.DangerLevel, out var lvl) ? lvl : 0,
            summary: b.MainText ?? string.Empty,
            problems: b.AvalancheProblems ?? string.Empty);
    }
}

internal sealed record VarsomBulletin
{
    [JsonPropertyName("DangerLevel")] public string? DangerLevel { get; init; }
    [JsonPropertyName("MainText")] public string? MainText { get; init; }
    [JsonPropertyName("AvalancheProblems")] public string? AvalancheProblems { get; init; }
}

public sealed class VarsomOptions
{
    public bool Enabled { get; set; }
    public string BaseUrl { get; set; } = "https://api.varsom.no/";
}
