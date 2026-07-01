using Npgsql;

namespace Turbo.Hosting.Postgres;

/// <summary>
/// Caps the Npgsql connection-pool size baked into a connection string.
///
/// Npgsql pools per distinct connection string, and its default
/// <c>Maximum Pool Size</c> is 100. In the modulith every module owns its own
/// database (a distinct connection string), so the process would default to
/// up to <c>100 × module-count</c> connections against a single CNPG instance
/// whose <c>max_connections</c> is 100 — a connection-exhaustion foot-gun on a
/// small, single-node box. Capping each pool to a small ceiling keeps the total
/// well under the server limit and trims idle-connection memory.
///
/// Only applied when the connection string does not already pin
/// <c>Maximum Pool Size</c> — an explicit value in the string always wins.
/// </summary>
public static class NpgsqlPoolSizing
{
    /// <summary>
    /// Returns <paramref name="connectionString"/> with <c>Maximum Pool Size</c>
    /// set to <paramref name="maxPoolSize"/> unless it is already specified.
    /// Leaves the string untouched (returns it verbatim) if it is null/blank or
    /// cannot be parsed as an Npgsql connection string.
    /// </summary>
    public static string WithMaxPoolSize(string? connectionString, int maxPoolSize)
    {
        if (string.IsNullOrWhiteSpace(connectionString))
        {
            return connectionString ?? string.Empty;
        }

        NpgsqlConnectionStringBuilder builder;
        try
        {
            builder = new NpgsqlConnectionStringBuilder(connectionString);
        }
        catch (ArgumentException)
        {
            // Not a parseable Npgsql connection string — leave it alone.
            return connectionString;
        }

        // Respect an explicit ceiling already present in the string.
        if (builder.ContainsKey("Maximum Pool Size"))
        {
            return connectionString;
        }

        builder.MaxPoolSize = maxPoolSize;
        return builder.ConnectionString;
    }
}
