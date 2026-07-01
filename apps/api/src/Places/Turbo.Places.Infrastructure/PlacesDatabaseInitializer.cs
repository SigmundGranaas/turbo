using Npgsql;
using Turboapi.Places.Core;

namespace Turboapi.Places.Infrastructure;

/// <summary>
/// Startup initialisation for the Places database. Places has no EF context
/// (the spatial read path is raw SQL), so it can't ride
/// <c>MigrateModuleDatabaseAsync</c> — this mirrors that helper's behaviour:
/// create the database if missing (skipping when it already exists so a
/// CREATEDB-less production role doesn't fail), then apply the idempotent
/// schema DDL via <see cref="PgPlaceStore.EnsureSchemaAsync"/>.
/// </summary>
public static class PlacesDatabaseInitializer
{
    public static async Task InitializeAsync(string connectionString, CancellationToken ct = default)
    {
        await EnsureDatabaseExistsAsync(connectionString, ct);
        // Build the store WITH the ruleset-derived prominence map so
        // EnsureSchemaAsync populates places.kind_prominence — the DB-side
        // retrieval prior is inert if this table is left empty.
        var ruleset = new RulesetProvider();
        await new PgPlaceStore(connectionString, ruleset.KindBonusMeters, ruleset.DefaultBonusMeters)
            .EnsureSchemaAsync(ct);
    }

    private static async Task EnsureDatabaseExistsAsync(string connectionString, CancellationToken ct)
    {
        var builder = new NpgsqlConnectionStringBuilder(connectionString);
        var targetDb = builder.Database
            ?? throw new InvalidOperationException("ConnectionStrings:Places has no Database= entry");
        builder.Database = "postgres";

        await using var conn = new NpgsqlConnection(builder.ConnectionString);
        await OpenWithRetryAsync(conn, ct);

        await using (var check = conn.CreateCommand())
        {
            check.CommandText = "SELECT 1 FROM pg_database WHERE datname = @db";
            check.Parameters.AddWithValue("db", targetDb);
            if (await check.ExecuteScalarAsync(ct) is not null) return;
        }

        await using var create = conn.CreateCommand();
        create.CommandText = $"CREATE DATABASE \"{targetDb.Replace("\"", "\"\"")}\"";
        try
        {
            await create.ExecuteNonQueryAsync(ct);
        }
        catch (PostgresException ex) when (ex.SqlState == "42P04")
        {
            // duplicate_database — created concurrently; nothing to do.
        }
    }

    private static async Task OpenWithRetryAsync(NpgsqlConnection conn, CancellationToken ct)
    {
        var delays = new[] { 0.5, 1, 2, 3, 5, 8 };
        for (var attempt = 0; ; attempt++)
        {
            try
            {
                await conn.OpenAsync(ct);
                return;
            }
            catch (NpgsqlException) when (attempt < delays.Length)
            {
                await Task.Delay(TimeSpan.FromSeconds(delays[attempt]), ct);
            }
        }
    }
}
