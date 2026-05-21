using Npgsql;

namespace Turbo.Behaviour.Testing;

/// <summary>
/// Locates module artefacts (content roots, connection-string helpers)
/// relative to the test assembly. Fixtures use this rather than hardcoding
/// paths.
/// </summary>
public static class RepoLayout
{
    /// <summary>
    /// Returns the bin/Debug/netNN.0 directory of the assembly that
    /// declares <typeparamref name="THostMarker"/>. WebApplicationFactory's
    /// content-root resolution doesn't reliably find the host's
    /// appsettings.Test.json when the marker lives in a different
    /// assembly from the test; passing this to UseContentRoot fixes that.
    /// </summary>
    public static string HostContentRoot<THostMarker>()
        => Path.GetDirectoryName(typeof(THostMarker).Assembly.Location)!;

    /// <summary>
    /// Creates a Postgres database against the given base connection string.
    /// Used by the modulith and microservice topology fixtures, which host
    /// multiple per-module databases inside a single Testcontainers Postgres.
    /// </summary>
    public static async Task CreateDatabaseAsync(string baseConnectionString, string databaseName)
    {
        await using var conn = new NpgsqlConnection(baseConnectionString);
        await conn.OpenAsync();
        await using var cmd = new NpgsqlCommand($"CREATE DATABASE \"{databaseName}\";", conn);
        await cmd.ExecuteNonQueryAsync();
    }

    /// <summary>
    /// Returns the input connection string with the <c>Database</c>
    /// parameter replaced by <paramref name="databaseName"/>.
    /// </summary>
    public static string WithDatabase(string baseConnectionString, string databaseName)
    {
        var b = new NpgsqlConnectionStringBuilder(baseConnectionString) { Database = databaseName };
        return b.ConnectionString;
    }
}
