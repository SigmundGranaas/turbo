using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Turboapi.Sharing.data;

namespace Turboapi.Sharing.integration;

public static class SharingBackfillExtensions
{
    /// <summary>
    /// Runs the Sharing backfill once. Reads payload-module connection
    /// strings from configuration (ConnectionStrings:Collections,
    /// :Geo, :Tracks); missing ones are skipped gracefully so the
    /// modulith can boot in deployment shapes that don't include every
    /// module.
    /// </summary>
    public static async Task BackfillSharingResourcesAsync(
        this IServiceProvider services,
        IConfiguration configuration,
        CancellationToken cancellationToken = default)
    {
        using var scope = services.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<SharingReadContext>();
        var logger = scope.ServiceProvider
            .GetRequiredService<ILoggerFactory>()
            .CreateLogger<SharingBackfillService>();

        var backfill = new SharingBackfillService(
            db,
            configuration.GetConnectionString("Collections"),
            configuration.GetConnectionString("Geo"),
            configuration.GetConnectionString("Tracks"),
            logger);
        await backfill.RunAsync(cancellationToken);
    }
}
