using System.Globalization;
using System.Net.Http;
using System.Xml;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Sehavnivå (api.sehavniva.no, Kartverket) tide provider. Fetches a
/// short window of observed + forecast water levels around the
/// requested instant and projects them into the typed
/// <see cref="TideSlice"/>.
///
/// Wiring: registered only when <c>Sehavniva:Enabled=true</c>;
/// otherwise <see cref="SyntheticTideProvider"/> is wired in its
/// place.
/// </summary>
public sealed class SehavnivaTideProvider : ITideProvider
{
    public const string HttpClientName = "sehavniva";

    public string Key => "sehavniva_tide";

    private readonly IHttpClientFactory _http;
    private readonly ILogger<SehavnivaTideProvider> _logger;

    public SehavnivaTideProvider(IHttpClientFactory http, ILogger<SehavnivaTideProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<TideSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var client = _http.CreateClient(HttpClientName);
        var from = at.UtcDateTime.AddMinutes(-15).ToString("yyyy-MM-ddTHH:mm");
        var to = at.UtcDateTime.AddMinutes(60).ToString("yyyy-MM-ddTHH:mm");

        // tideapi.php returns observed + forecast water levels relative
        // to several datums. We request "all" data sources and pick the
        // one closest to "now".
        var url = $"tideapi.php?lat={Math.Round(latitude, 4)}&lon={Math.Round(longitude, 4)}"
                  + $"&fromtime={Uri.EscapeDataString(from)}&totime={Uri.EscapeDataString(to)}"
                  + $"&datatype=all&refcode=cd&place=&file=&lang=en&interval=10&dst=0&tzone=&tide_request=locationdata";

        List<(DateTimeOffset Time, float Value)> points;
        try
        {
            await using var stream = await client.GetStreamAsync(url, cancellationToken);
            points = ParseWaterlevels(stream);
        }
        catch (HttpRequestException ex)
        {
            throw new ConditionsProviderException("Sehavnivå upstream request failed", ex);
        }
        catch (XmlException ex)
        {
            throw new ConditionsProviderException("Sehavnivå returned malformed XML", ex);
        }
        if (points.Count == 0)
            throw new ConditionsProviderException("Sehavnivå returned no waterlevel entries");

        // Closest to `at` for "current", and one ~15min later for trend.
        points.Sort((a, b) => a.Time.CompareTo(b.Time));
        var current = points.OrderBy(p => Math.Abs((p.Time - at).TotalSeconds)).First();
        var aheadIdx = points.FindIndex(p => p.Time > current.Time);

        string summary;
        if (aheadIdx < 0) summary = "current level unchanged";
        else
        {
            var ahead = points[aheadIdx];
            var dh = ahead.Value - current.Value;
            summary = Math.Abs(dh) < 0.02
                ? "slack"
                : dh > 0 ? "rising tide" : "falling tide";
        }

        return new TideSlice(
            validAt: current.Time,
            currentHeightMeters: current.Value,
            summary: summary);
    }

    internal static List<(DateTimeOffset Time, float Value)> ParseWaterlevels(Stream xmlStream)
    {
        var settings = new XmlReaderSettings
        {
            DtdProcessing = DtdProcessing.Prohibit,
            XmlResolver = null,
            IgnoreComments = true,
            IgnoreWhitespace = true,
            CloseInput = false,
        };
        var points = new List<(DateTimeOffset, float)>();
        using var reader = XmlReader.Create(xmlStream, settings);
        while (reader.Read())
        {
            if (reader.NodeType != XmlNodeType.Element) continue;
            if (!reader.Name.Equals("waterlevel", StringComparison.OrdinalIgnoreCase)) continue;

            var valueAttr = reader.GetAttribute("value");
            var timeAttr = reader.GetAttribute("time");
            if (valueAttr is null || timeAttr is null) continue;

            if (!float.TryParse(valueAttr, NumberStyles.Float, CultureInfo.InvariantCulture, out var v))
                continue;
            if (!DateTimeOffset.TryParse(
                    timeAttr, CultureInfo.InvariantCulture,
                    DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal,
                    out var t))
                continue;

            points.Add((t, v));
        }
        return points;
    }
}

public sealed class SehavnivaOptions
{
    public bool Enabled { get; set; }
    public string BaseUrl { get; set; } = "https://api.sehavniva.no/";
}
