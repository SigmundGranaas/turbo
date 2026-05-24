using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Npgsql;

namespace Turbo.Hosting.Postgres;

/// <summary>
/// Host startup helpers for per-module Postgres databases. EF Core's
/// <see cref="DatabaseFacade.MigrateAsync(System.Threading.CancellationToken)"/>
/// only runs migrations against an existing database — it does not
/// <c>CREATE DATABASE</c>. <see cref="MigrateModuleDatabaseAsync{TContext}"/>
/// fills that gap: it opens a connection to the cluster-level
/// <c>postgres</c> database, creates the target DB if missing, then runs
/// <c>MigrateAsync</c> against the module's context.
///
/// Concurrent host replicas are safe — EF Core acquires an exclusive lock
/// on <c>__EFMigrationsHistory</c> for the duration of <c>MigrateAsync</c>;
/// runners serialise.
/// </summary>
public static class DatabaseInitialization
{
    /// <summary>
    /// Ensures the target Postgres database exists and runs all pending
    /// EF Core migrations for <typeparamref name="TContext"/>.
    /// </summary>
    /// <param name="services">Host service provider.</param>
    /// <param name="connectionString">
    ///   Full connection string including username/password. Hosts read this
    ///   from configuration (<c>ConnectionStrings:&lt;Module&gt;</c>); test
    ///   fixtures pass the Testcontainers connection string directly.
    /// </param>
    public static Task MigrateModuleDatabaseAsync<TContext>(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        where TContext : DbContext
        => MigrateModuleDatabaseAsync<TContext>(services, connectionString, schema: null, cancellationToken);

    /// <summary>
    /// Module-scoped overload: also ensures <paramref name="schema"/>
    /// exists before EF runs migrations. Required when a module pins its
    /// <c>__EFMigrationsHistory</c> table to a non-default schema so that
    /// multiple modules can share one database without colliding.
    /// </summary>
    public static async Task MigrateModuleDatabaseAsync<TContext>(
        this IServiceProvider services,
        string connectionString,
        string? schema,
        CancellationToken cancellationToken = default)
        where TContext : DbContext
    {
        await using var scope = services.CreateAsyncScope();
        var ctx = scope.ServiceProvider.GetRequiredService<TContext>();
        var logger = scope.ServiceProvider.GetService<ILogger<TContext>>();

        var builder = new NpgsqlConnectionStringBuilder(connectionString);
        var targetDb = builder.Database
            ?? throw new InvalidOperationException(
                $"Connection string for {typeof(TContext).Name} has no Database= entry");

        await EnsureDatabaseExistsAsync(builder, targetDb, logger, cancellationToken);
        if (!string.IsNullOrWhiteSpace(schema))
        {
            await EnsureSchemaExistsAsync(connectionString, schema!, logger, cancellationToken);
        }
        // Pre-create the migrations-history table so EF's first-boot
        // probe doesn't log a CommandError when it can't find it. Same
        // shape EF would create on demand, just done up front in a
        // CREATE TABLE IF NOT EXISTS so the boot logs stay clean.
        await EnsureMigrationsHistoryTableExistsAsync(
            connectionString, schema ?? "public", logger, cancellationToken);
        logger?.LogInformation("Running EF Core migrations for {Context} → {Database}{Schema}",
            typeof(TContext).Name, targetDb, schema is null ? "" : $" (schema {schema})");
        await ctx.Database.MigrateAsync(cancellationToken);
    }

    private static async Task EnsureDatabaseExistsAsync(
        NpgsqlConnectionStringBuilder builder,
        string targetDb,
        ILogger? logger,
        CancellationToken cancellationToken)
    {
        // Connect to the cluster's default 'postgres' DB to issue CREATE DATABASE.
        builder.Database = "postgres";
        await using var conn = new NpgsqlConnection(builder.ConnectionString);
        // The host can race a Postgres container that's still starting
        // — compose has a healthcheck but no depends_on chain hides it
        // from a single-process host. Short retry budget so the first
        // boot of a fresh docker-compose stack succeeds without making
        // every operator add explicit `depends_on` per topology.
        await OpenWithRetryAsync(conn, logger, cancellationToken);

        await using var cmd = conn.CreateCommand();
        // Identifier must be inlined, not parameterised — Postgres does not
        // allow parameters in CREATE DATABASE.
        cmd.CommandText = $"CREATE DATABASE \"{targetDb.Replace("\"", "\"\"")}\"";
        try
        {
            await cmd.ExecuteNonQueryAsync(cancellationToken);
            logger?.LogInformation("Created database {Database}", targetDb);
        }
        catch (PostgresException ex) when (ex.SqlState == "42P04")
        {
            // 42P04 = duplicate_database; already exists, nothing to do.
        }
    }

    private static async Task EnsureSchemaExistsAsync(
        string connectionString,
        string schema,
        ILogger? logger,
        CancellationToken cancellationToken)
    {
        await using var conn = new NpgsqlConnection(connectionString);
        await OpenWithRetryAsync(conn, logger, cancellationToken);
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = $"CREATE SCHEMA IF NOT EXISTS \"{schema.Replace("\"", "\"\"")}\"";
        await cmd.ExecuteNonQueryAsync(cancellationToken);
        logger?.LogDebug("Ensured schema {Schema}", schema);
    }

    private static async Task EnsureMigrationsHistoryTableExistsAsync(
        string connectionString,
        string schema,
        ILogger? logger,
        CancellationToken cancellationToken)
    {
        await using var conn = new NpgsqlConnection(connectionString);
        await OpenWithRetryAsync(conn, logger, cancellationToken);
        await using var cmd = conn.CreateCommand();
        var qs = schema.Replace("\"", "\"\"");
        cmd.CommandText = $"""
            CREATE TABLE IF NOT EXISTS "{qs}"."__EFMigrationsHistory" (
                "MigrationId" varchar(150) NOT NULL,
                "ProductVersion" varchar(32) NOT NULL,
                CONSTRAINT "PK___EFMigrationsHistory" PRIMARY KEY ("MigrationId")
            );
            """;
        await cmd.ExecuteNonQueryAsync(cancellationToken);
    }

    private static async Task OpenWithRetryAsync(
        NpgsqlConnection conn,
        ILogger? logger,
        CancellationToken cancellationToken)
    {
        var delays = new[]
        {
            TimeSpan.FromMilliseconds(500),
            TimeSpan.FromSeconds(1),
            TimeSpan.FromSeconds(2),
            TimeSpan.FromSeconds(3),
            TimeSpan.FromSeconds(5),
            TimeSpan.FromSeconds(8),
            TimeSpan.FromSeconds(13),
        };
        for (var attempt = 0; ; attempt++)
        {
            try
            {
                await conn.OpenAsync(cancellationToken);
                return;
            }
            catch (NpgsqlException ex) when (attempt < delays.Length)
            {
                logger?.LogDebug(
                    "Postgres not ready (attempt {Attempt}/{Max}): {Message} — retrying in {Delay}",
                    attempt + 1, delays.Length, ex.Message, delays[attempt]);
                await Task.Delay(delays[attempt], cancellationToken);
            }
        }
    }
}
