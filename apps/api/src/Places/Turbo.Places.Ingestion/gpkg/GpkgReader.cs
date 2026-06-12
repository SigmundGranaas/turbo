using Microsoft.Data.Sqlite;
using NetTopologySuite.Geometries;

namespace Turboapi.Places.Ingestion;

/// <summary>One feature read from a GeoPackage table: its geometry (in the
/// file's source CRS — reproject with <see cref="Utm33"/>) plus the requested
/// attribute columns as strings.</summary>
public sealed record GpkgFeature(Geometry Geometry, IReadOnlyDictionary<string, string?> Attributes);

/// <summary>
/// Streams features from a GeoPackage (SQLite) feature table without GDAL.
/// Geometry blobs are decoded by <see cref="GpkgGeometry"/>; rows with a null
/// geometry are skipped. The caller supplies the table, geometry column, and
/// attribute columns (the SSR/Matrikkel schemas are known per dataset).
/// </summary>
public sealed class GpkgReader
{
    public IEnumerable<GpkgFeature> ReadFeatures(
        string path, string table, string geometryColumn, IReadOnlyList<string> attributeColumns)
    {
        using var conn = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = path,
            Mode = SqliteOpenMode.ReadOnly,
        }.ToString());
        conn.Open();

        using var cmd = conn.CreateCommand();
        var columns = string.Join(", ",
            new[] { geometryColumn }.Concat(attributeColumns).Select(Quote));
        cmd.CommandText = $"SELECT {columns} FROM {Quote(table)}";

        using var reader = cmd.ExecuteReader();
        while (reader.Read())
        {
            if (reader.IsDBNull(0)) continue;

            using var blobStream = reader.GetStream(0);
            using var ms = new MemoryStream();
            blobStream.CopyTo(ms);
            var geometry = GpkgGeometry.Parse(ms.ToArray());

            var attributes = new Dictionary<string, string?>(attributeColumns.Count);
            for (var i = 0; i < attributeColumns.Count; i++)
            {
                attributes[attributeColumns[i]] =
                    reader.IsDBNull(i + 1) ? null : reader.GetValue(i + 1)?.ToString();
            }

            yield return new GpkgFeature(geometry, attributes);
        }
    }

    private static string Quote(string identifier) => $"\"{identifier.Replace("\"", "\"\"")}\"";
}
