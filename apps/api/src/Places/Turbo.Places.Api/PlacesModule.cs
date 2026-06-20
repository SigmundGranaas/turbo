using System.Threading.RateLimiting;
using Microsoft.AspNetCore.RateLimiting;
using Turboapi.Places.controller;
using Turboapi.Places.Core;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places;

/// <summary>
/// Composition entry point for the Places module: the read-only reference-data
/// service (search + reverse geocoding) over the owned Kartverket datasets.
/// Unlike the payload modules there is no aggregate/outbox — the writer is the
/// ingestion job; this module is purely the query side.
/// </summary>
public static class PlacesModule
{
    public const string ConnectionStringName = "Places";

    /// <summary>Per-IP rate-limit policy applied to every Places endpoint. The
    /// module owns it so the protection travels into any host (the modulith has
    /// no gateway in front), not just the standalone host.</summary>
    public const string RateLimitPolicy = "places";

    public static IServiceCollection AddPlacesModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);

        // Stateless services over a read-only dataset: singletons, no scopes.
        services.AddSingleton<IPlaceStore>(_ => new PgPlaceStore(connectionString));
        services.AddSingleton<ReverseGeocodeService>();
        services.AddSingleton<SearchService>();

        AddPlacesRateLimiter(services, configuration);

        // ETag source: cache the active dataset version in-process so the hot
        // path never scans places. 0 disables the cache (tests see publishes
        // immediately); production default 5 s.
        var ttl = TimeSpan.FromSeconds(configuration.GetValue("Places:VersionCacheSeconds", 5.0));
        services.AddSingleton(sp => new DatasetVersionProvider(
            sp.GetRequiredService<IPlaceStore>(), ttl));
        services.AddSingleton<RulesetProvider>();
        services.AddSingleton(_ => new BundleBuilder(connectionString));

        // Nasjonal Turbase (ut.no / DNT) proxy: a typed HttpClient that injects
        // the server-held api key. Bound from the `Turbasen` config section.
        services.Configure<TurbasenConfig>(configuration.GetSection("Turbasen"));
        services.AddHttpClient<NasjonalTurbaseProxyClient>();

        services.AddControllers().AddApplicationPart(typeof(PlacesController).Assembly);

        return services;
    }

    /// <summary>
    /// A defense-in-depth per-client fixed-window limit (the gateway/ingress is
    /// the primary guard). The partition key is the real client IP from
    /// <c>X-Forwarded-For</c> (left-most) — behind k8s ingress
    /// <c>RemoteIpAddress</c> is the proxy, which would bucket everyone
    /// together. <c>Places:RateLimitPermitPerWindow</c> &lt;= 0 disables it.
    /// </summary>
    private static void AddPlacesRateLimiter(IServiceCollection services, IConfiguration configuration)
    {
        var permit = configuration.GetValue("Places:RateLimitPermitPerWindow", 600);
        var windowSeconds = configuration.GetValue("Places:RateLimitWindowSeconds", 60);

        services.AddRateLimiter(options =>
        {
            options.RejectionStatusCode = StatusCodes.Status429TooManyRequests;
            options.AddPolicy(RateLimitPolicy, ctx =>
            {
                if (permit <= 0)
                    return RateLimitPartition.GetNoLimiter("disabled");
                return RateLimitPartition.GetFixedWindowLimiter(ClientKey(ctx), _ =>
                    new FixedWindowRateLimiterOptions
                    {
                        PermitLimit = permit,
                        Window = TimeSpan.FromSeconds(windowSeconds),
                        QueueLimit = 0,
                    });
            });
        });
    }

    /// <summary>The originating client IP: the left-most <c>X-Forwarded-For</c>
    /// hop if present (set by the ingress), else the socket peer.</summary>
    private static string ClientKey(HttpContext ctx)
    {
        var forwarded = ctx.Request.Headers["X-Forwarded-For"].ToString();
        if (!string.IsNullOrEmpty(forwarded))
        {
            var first = forwarded.Split(',')[0].Trim();
            if (first.Length > 0) return first;
        }
        return ctx.Connection.RemoteIpAddress?.ToString() ?? "anon";
    }

    /// <summary>
    /// Startup hook (the Places analogue of <c>MigrateModuleDatabaseAsync</c>):
    /// fails fast if the native ranking core isn't loadable, then creates the
    /// database if missing and applies the idempotent schema DDL.
    /// </summary>
    public static Task InitializePlacesModuleAsync(
        this IServiceProvider _, string connectionString, CancellationToken ct = default)
    {
        Turboapi.Places.Core.PlacesStartupProbe.Verify();
        return PlacesDatabaseInitializer.InitializeAsync(connectionString, ct);
    }

    public static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
