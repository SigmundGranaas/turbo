using NetTopologySuite.Geometries;
using NetTopologySuite.IO;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// Decodes a GeoPackageBinary geometry blob (GeoPackage spec §2.1.3): a small
/// header — magic "GP", version, flags (byte-order + envelope type + empty),
/// srs_id, optional envelope — followed by standard WKB. We strip the header
/// and delegate the WKB to NetTopologySuite (which reads its own byte-order
/// byte), so no GDAL is needed.
/// </summary>
public static class GpkgGeometry
{
    public static Geometry Parse(byte[] gpb)
    {
        if (gpb is null || gpb.Length < 8 || gpb[0] != (byte)'G' || gpb[1] != (byte)'P')
            throw new FormatException("Not a GeoPackageBinary geometry blob (missing 'GP' magic).");

        var flags = gpb[3];
        // Bit 0: header byte order (doesn't affect WKB). Bits 1-3: envelope code.
        var envelopeCode = (flags >> 1) & 0x07;
        var envelopeDoubles = envelopeCode switch
        {
            0 => 0,   // no envelope
            1 => 4,   // [minX,maxX,minY,maxY]
            2 => 6,   // + [minZ,maxZ]
            3 => 6,   // + [minM,maxM]
            4 => 8,   // + Z and M
            _ => throw new FormatException($"Reserved GeoPackage envelope code {envelopeCode}."),
        };

        var headerLength = 8 + envelopeDoubles * 8;
        if (gpb.Length <= headerLength)
            throw new FormatException("GeoPackage blob has no WKB payload.");

        var wkb = new ReadOnlySpan<byte>(gpb, headerLength, gpb.Length - headerLength).ToArray();
        return new WKBReader().Read(wkb);
    }
}
