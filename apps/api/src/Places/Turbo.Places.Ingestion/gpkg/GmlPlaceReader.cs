using System.Globalization;
using System.Xml;
using System.Xml.Linq;
using Turboapi.Places;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// Streams the SSR (Sentralt stedsnavnregister, <c>StedsnavnForVanligBruk</c>)
/// GML into canonical <see cref="Place"/>s without GDAL. The national download
/// is GML-only and landsdekkende, so the reader is forward-only: an
/// <see cref="XmlReader"/> walks to each <c>app:Sted</c> and lifts just that
/// element into an <see cref="XElement"/> to parse, keeping memory bounded at
/// one feature regardless of file size.
///
/// Both source CRSs are handled by branching on the geometry's <c>srsName</c>:
/// the WFS ships EPSG:4258 (geographic, <c>lat lon</c>), the national file
/// EPSG:25833 (projected UTM33, <c>easting northing</c>) which is reprojected
/// with <see cref="Utm33"/>. Names are folded/rejected by the shared
/// <see cref="Normalization"/> so bulk rows match the REST-sampling path.
/// </summary>
public sealed class GmlPlaceReader
{
    public IEnumerable<Place> ReadPlaces(string path, string source = "ssr")
    {
        using var stream = File.OpenRead(path);
        foreach (var place in ReadPlaces(stream, source))
            yield return place;
    }

    public IEnumerable<Place> ReadPlaces(Stream stream, string source = "ssr")
    {
        var settings = new XmlReaderSettings { IgnoreComments = true, IgnoreWhitespace = true };
        using var reader = XmlReader.Create(stream, settings);

        while (reader.Read())
        {
            if (reader.NodeType != XmlNodeType.Element || reader.LocalName != "Sted")
                continue;

            // Lift only this feature's subtree — never the whole document.
            if (XNode.ReadFrom(reader) is not XElement sted) continue;
            var place = Map(sted, source);
            if (place is not null) yield return place;
        }
    }

    private static Place? Map(XElement sted, string source)
    {
        var name = PrimaryName(sted);
        if (!Normalization.IsUsableName(name)) return null;

        var id = Local(sted, "stedsnummer") ?? Local(sted, "lokalId");
        if (string.IsNullOrWhiteSpace(id)) return null;

        var point = LabelPoint(sted);
        if (point is null) return null;

        var kind = Local(sted, "navneobjekttype") ?? "";
        // SSR ForVanligBruk only carries active names; honour an explicit
        // stedstatus if a future extract ships one.
        var status = Local(sted, "stedstatus") ?? "aktiv";
        var kommune = Local(sted, "kommunenavn");
        var fylke = Local(sted, "fylkesnavn");

        return new Place(source, id!, kind, name!.Trim(), point.Value.Lat, point.Value.Lng, status,
            KommuneName: kommune, FylkeName: fylke);
    }

    /// <summary>The display name: among the place's <c>Stedsnavn</c>, prefer
    /// <c>hovednavn</c> and the lowest <c>stedsnavnnummer</c> (a multilingual
    /// place lists each language as its own hovednavn).</summary>
    private static string? PrimaryName(XElement sted)
    {
        var best = sted.Descendants().Where(e => e.Name.LocalName == "Stedsnavn")
            .Select(n => new
            {
                Name = Local(n, "komplettskrivemåte"),
                IsMain = string.Equals(Local(n, "navnestatus"), "hovednavn", StringComparison.OrdinalIgnoreCase),
                Order = int.TryParse(Local(n, "stedsnavnnummer"), out var o) ? o : int.MaxValue,
            })
            .Where(n => !string.IsNullOrWhiteSpace(n.Name))
            .OrderByDescending(n => n.IsMain)
            .ThenBy(n => n.Order)
            .FirstOrDefault();
        return best?.Name;
    }

    /// <summary>First vertex of the place's geometry, reprojected to WGS84.
    /// Works for every SSR geometry container — Point, MultiPoint (national
    /// download), LineString/Curve and Surface — by reading the first
    /// <c>gml:pos</c>/<c>posList</c> and resolving <c>srsName</c> from the
    /// nearest ancestor that declares it (the download GML puts srsName on the
    /// MultiPoint/Surface, not on each inner Point).</summary>
    private static (double Lat, double Lng)? LabelPoint(XElement sted)
    {
        var coord = sted.Descendants()
            .FirstOrDefault(e => e.Name.LocalName is "pos" or "posList");
        if (coord is null || string.IsNullOrWhiteSpace(coord.Value)) return null;

        var tokens = coord.Value.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
        if (tokens.Length < 2) return null;
        if (!double.TryParse(tokens[0], NumberStyles.Float, CultureInfo.InvariantCulture, out var a) ||
            !double.TryParse(tokens[1], NumberStyles.Float, CultureInfo.InvariantCulture, out var b))
            return null;

        if (IsProjectedUtm(Srs(coord)))
            return Utm33.ToWgs84(a, b); // projected: easting northing
        return (a, b);                  // geographic (4258/4326): lat lon
    }

    /// <summary>The effective <c>srsName</c> for a coordinate element: its own
    /// or the nearest ancestor's (the geometry container carries it).</summary>
    private static string Srs(XElement coord)
    {
        for (var e = coord; e is not null; e = e.Parent)
        {
            var srs = e.Attribute("srsName")?.Value;
            if (!string.IsNullOrEmpty(srs)) return srs;
        }
        return "";
    }

    private static bool IsProjectedUtm(string srs) =>
        srs.Contains("25832") || srs.Contains("25833") || srs.Contains("25835");

    private static string? Local(XElement root, string localName) =>
        root.Descendants().FirstOrDefault(e => e.Name.LocalName == localName)?.Value;
}
