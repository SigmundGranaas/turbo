using Microsoft.Data.Sqlite;

namespace Turbo.Places.Behaviour;

/// <summary>
/// Builds minimal GeoPackage feature tables (raw SQLite + hand-assembled GPB
/// blobs, per the GeoPackage spec) for tests — independent of the production
/// reader.
/// </summary>
internal static class TestGpkg
{
    /// <summary>GeoPackageBinary for a 2D point: LE header, no envelope, srs_id,
    /// then LE WKB point (spec §2.1.3).</summary>
    public static byte[] PointBlob(double x, double y, int srsId = 25833)
    {
        using var ms = new MemoryStream();
        ms.WriteByte((byte)'G');
        ms.WriteByte((byte)'P');
        ms.WriteByte(0);     // version
        ms.WriteByte(0x01);  // flags: LE header, envelope code 0 (none)
        ms.Write(BitConverter.GetBytes(srsId));
        ms.WriteByte(0x01);                       // WKB byte order: LE
        ms.Write(BitConverter.GetBytes((uint)1)); // WKB type: Point
        ms.Write(BitConverter.GetBytes(x));
        ms.Write(BitConverter.GetBytes(y));
        return ms.ToArray();
    }

    /// <summary>Creates <paramref name="table"/> with a geom BLOB + the given
    /// TEXT columns and inserts the rows (geometry first, then column values).</summary>
    public static void Write(
        string path, string table, string[] textColumns,
        IEnumerable<(byte[] Geom, string?[] Values)> rows)
    {
        using var conn = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = path,
            Mode = SqliteOpenMode.ReadWriteCreate,
        }.ToString());
        conn.Open();

        var cols = string.Join(", ", textColumns.Select(c => $"{c} TEXT"));
        using (var ddl = conn.CreateCommand())
        {
            ddl.CommandText = $"CREATE TABLE {table} (fid INTEGER PRIMARY KEY, geom BLOB, {cols});";
            ddl.ExecuteNonQuery();
        }

        foreach (var (geom, values) in rows)
        {
            using var ins = conn.CreateCommand();
            var names = string.Join(", ", textColumns);
            var args = string.Join(", ", textColumns.Select((_, i) => $"$v{i}"));
            ins.CommandText = $"INSERT INTO {table} (geom, {names}) VALUES ($g, {args})";
            ins.Parameters.AddWithValue("$g", geom);
            for (var i = 0; i < textColumns.Length; i++)
                ins.Parameters.AddWithValue($"$v{i}", (object?)values[i] ?? DBNull.Value);
            ins.ExecuteNonQuery();
        }
        SqliteConnection.ClearAllPools();
    }
}
