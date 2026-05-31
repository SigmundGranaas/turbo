using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Http.Resilience;
using Microsoft.Extensions.Options;
using Turbo.Hosting.Postgres;
using Turbo.Messaging;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.conditions;
using Turboapi.Activities.controller;
using Turboapi.Activities.data;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities;

/// <summary>
/// Composition entry point for the Activities shared module. Owns the
/// cross-kind summaries database, the kind catalog facade, the shared
/// domain services (geometry normalisation, owner guard), and the summary
/// projection subscribers. Per-kind modules (e.g. AddFishingActivityModule)
/// are registered separately by the host.
/// </summary>
public static class ActivitiesSharedModule
{
    private const string Schema = "activities";

    public static IServiceCollection AddActivitiesSharedModule(
        this IServiceCollection services,
        IConfiguration configuration,
        string connectionString)
    {
        services.AddDbContext<ActivitySummariesContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.MigrationsHistoryTable("__EFMigrationsHistory", Schema);
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        // Shared domain services. Composition root: every kind handler picks
        // these up from DI rather than inheriting any base class.
        services.AddSingleton<IGeometryNormalizer, GeometryNormalizer>();
        services.AddSingleton<IOwnerGuard, OwnerGuard>();
        services.AddSingleton<IActivityKindCatalog, InMemoryActivityKindCatalog>();

        // The summaries projection is its own outbox/UnitOfWork scope so the
        // projector can write to the summaries table + processed-events
        // atomically. Kind modules have their own scopes.
        services.AddScoped<IOutbox<ActivitiesScope>, PgOutbox<ActivitySummariesContext, ActivitiesScope>>();
        services.AddScoped<IUnitOfWork<ActivitiesScope>, PgUnitOfWork<ActivitySummariesContext, ActivitiesScope>>();
        services.AddScoped<IIdempotencyStore<ActivitySummariesContext>, PgIdempotencyStore<ActivitySummariesContext>>();

        services.AddScoped<ActivitySummaryUpsertedHandler>();
        services.AddScoped<ActivitySummaryDeletedHandler>();
        services.AddScoped<IEventHandler<ActivitySummaryUpserted>>(sp =>
            sp.GetRequiredService<ActivitySummaryUpsertedHandler>());
        services.AddScoped<IEventHandler<ActivitySummaryDeleted>>(sp =>
            sp.GetRequiredService<ActivitySummaryDeletedHandler>());

        services.AddHostedService<OutboxDispatcherHostedService<ActivitySummariesContext>>();

        // Conditions cache + weather provider chain. Composition: per-kind
        // advisors take an IWeatherProvider; the registration here picks
        // SyntheticWeatherProvider out of the box, swapping in
        // MetNoWeatherProvider when MetNo:UserAgent is set in
        // configuration. Either way the consumer is wrapped in
        // CachedWeatherProvider so all kinds share one Postgres-backed
        // cache.
        services.AddScoped<IConditionsCache, PgConditionsCache>();
        services.Configure<MetNoOptions>(configuration.GetSection("MetNo"));

        var metNoUserAgent = configuration["MetNo:UserAgent"];
        if (!string.IsNullOrWhiteSpace(metNoUserAgent))
        {
            services.AddHttpClient(MetNoWeatherProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<MetNoOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.DefaultRequestHeaders.UserAgent.ParseAdd(opts.UserAgent!);
                http.Timeout = TimeSpan.FromSeconds(10);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddScoped<MetNoWeatherProvider>();
            services.AddScoped<IWeatherProvider>(sp =>
            {
                IWeatherProvider cached = new CachedWeatherProvider(
                    sp.GetRequiredService<MetNoWeatherProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedWeatherProvider>>());
                return new SnapshottingWeatherProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingWeatherProvider>>());
            });
            services.AddSingleton<IMetricExtractor>(new WeatherMetricExtractor("met_no_weather"));
        }
        else
        {
            services.AddSingleton<SyntheticWeatherProvider>();
            services.AddScoped<IWeatherProvider>(sp =>
            {
                IWeatherProvider cached = new CachedWeatherProvider(
                    sp.GetRequiredService<SyntheticWeatherProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedWeatherProvider>>());
                return new SnapshottingWeatherProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingWeatherProvider>>());
            });
            services.AddSingleton<IMetricExtractor>(new WeatherMetricExtractor("synthetic_weather"));
        }

        // Avalanche provider. Synthetic by default; VarsomAvalancheProvider
        // takes over when Varsom:Enabled=true is set.
        services.Configure<VarsomOptions>(configuration.GetSection("Varsom"));
        if (configuration.GetValue<bool>("Varsom:Enabled"))
        {
            services.AddHttpClient(VarsomAvalancheProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<VarsomOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.Timeout = TimeSpan.FromSeconds(10);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddScoped<VarsomAvalancheProvider>();
            services.AddScoped<IAvalancheProvider>(sp =>
            {
                IAvalancheProvider cached = new CachedAvalancheProvider(
                    sp.GetRequiredService<VarsomAvalancheProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedAvalancheProvider>>());
                return new SnapshottingAvalancheProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingAvalancheProvider>>());
            });
        }
        else
        {
            services.AddSingleton<SyntheticAvalancheProvider>();
            services.AddScoped<IAvalancheProvider>(sp =>
            {
                IAvalancheProvider cached = new CachedAvalancheProvider(
                    sp.GetRequiredService<SyntheticAvalancheProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedAvalancheProvider>>());
                return new SnapshottingAvalancheProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingAvalancheProvider>>());
            });
        }

        // River flow provider. Synthetic by default; NveRiverFlowProvider
        // takes over when Nve:ApiKey is set.
        services.Configure<NveOptions>(configuration.GetSection("Nve"));
        var nveApiKey = configuration["Nve:ApiKey"];
        if (!string.IsNullOrWhiteSpace(nveApiKey))
        {
            services.AddHttpClient(NveRiverFlowProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<NveOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.DefaultRequestHeaders.Add("X-API-Key", opts.ApiKey!);
                http.Timeout = TimeSpan.FromSeconds(10);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddScoped<NveRiverFlowProvider>();
            services.AddScoped<IRiverFlowProvider>(sp =>
            {
                IRiverFlowProvider cached = new CachedRiverFlowProvider(
                    sp.GetRequiredService<NveRiverFlowProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedRiverFlowProvider>>());
                return new SnapshottingRiverFlowProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingRiverFlowProvider>>());
            });
            services.AddSingleton<IMetricExtractor>(new RiverFlowMetricExtractor("nve_river_flow"));
        }
        else
        {
            services.AddSingleton<SyntheticRiverFlowProvider>();
            services.AddScoped<IRiverFlowProvider>(sp =>
            {
                IRiverFlowProvider cached = new CachedRiverFlowProvider(
                    sp.GetRequiredService<SyntheticRiverFlowProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedRiverFlowProvider>>());
                return new SnapshottingRiverFlowProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingRiverFlowProvider>>());
            });
            services.AddSingleton<IMetricExtractor>(new RiverFlowMetricExtractor("synthetic_river_flow"));
        }

        // Tide provider. Synthetic by default; SehavnivaTideProvider
        // takes over when Sehavniva:Enabled=true is set.
        services.Configure<SehavnivaOptions>(configuration.GetSection("Sehavniva"));
        if (configuration.GetValue<bool>("Sehavniva:Enabled"))
        {
            services.AddHttpClient(SehavnivaTideProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<SehavnivaOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.Timeout = TimeSpan.FromSeconds(10);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddScoped<SehavnivaTideProvider>();
            services.AddScoped<ITideProvider>(sp =>
            {
                ITideProvider cached = new CachedTideProvider(
                    sp.GetRequiredService<SehavnivaTideProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedTideProvider>>());
                return new SnapshottingTideProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingTideProvider>>());
            });
        }
        else
        {
            services.AddSingleton<SyntheticTideProvider>();
            services.AddScoped<ITideProvider>(sp =>
            {
                ITideProvider cached = new CachedTideProvider(
                    sp.GetRequiredService<SyntheticTideProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedTideProvider>>());
                return new SnapshottingTideProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingTideProvider>>());
            });
        }

        // Grooming provider. Synthetic by default; SkisporetGroomingProvider
        // takes over when Skisporet:Enabled=true is set.
        services.Configure<SkisporetOptions>(configuration.GetSection("Skisporet"));
        if (configuration.GetValue<bool>("Skisporet:Enabled"))
        {
            services.AddHttpClient(SkisporetGroomingProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<SkisporetOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.Timeout = TimeSpan.FromSeconds(10);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddScoped<SkisporetGroomingProvider>();
            services.AddScoped<IGroomingProvider>(sp =>
            {
                IGroomingProvider cached = new CachedGroomingProvider(
                    sp.GetRequiredService<SkisporetGroomingProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedGroomingProvider>>());
                return new SnapshottingGroomingProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingGroomingProvider>>());
            });
        }
        else
        {
            services.AddSingleton<SyntheticGroomingProvider>();
            services.AddScoped<IGroomingProvider>(sp =>
            {
                IGroomingProvider cached = new CachedGroomingProvider(
                    sp.GetRequiredService<SyntheticGroomingProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedGroomingProvider>>());
                return new SnapshottingGroomingProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingGroomingProvider>>());
            });
        }

        // Optional background warmer for the conditions cache. Off by
        // default (ConditionsCacheWarmer:Enabled=true to switch on).
        services.Configure<ConditionsCacheWarmerOptions>(configuration.GetSection("ConditionsCacheWarmer"));
        services.AddHostedService<ConditionsCacheWarmerHostedService>();

        // Elevation provider. Kartverket Høydedata when Kartverket:Enabled=true;
        // synthetic ramp otherwise.
        services.Configure<KartverketOptions>(configuration.GetSection("Kartverket"));
        if (configuration.GetValue<bool>("Kartverket:Enabled"))
        {
            services.AddHttpClient(KartverketElevationProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<KartverketOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.Timeout = TimeSpan.FromSeconds(8);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddSingleton<IElevationProvider, KartverketElevationProvider>();
        }
        else
        {
            services.AddSingleton<IElevationProvider, SyntheticElevationProvider>();
        }

        // Snowpack (regObs) provider. Real impl when RegObs:Enabled=true;
        // deterministic synthetic generator otherwise.
        services.Configure<RegObsOptions>(configuration.GetSection("RegObs"));
        if (configuration.GetValue<bool>("RegObs:Enabled"))
        {
            services.AddHttpClient(RegObsSnowpackProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<RegObsOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.DefaultRequestHeaders.UserAgent.ParseAdd(opts.UserAgent);
                http.Timeout = TimeSpan.FromSeconds(8);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddScoped<RegObsSnowpackProvider>();
            services.AddScoped<ISnowpackProvider>(sp =>
            {
                ISnowpackProvider cached = new CachedSnowpackProvider(
                    sp.GetRequiredService<RegObsSnowpackProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedSnowpackProvider>>());
                return new SnapshottingSnowpackProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingSnowpackProvider>>());
            });
        }
        else
        {
            services.AddSingleton<SyntheticSnowpackProvider>();
            services.AddScoped<ISnowpackProvider>(sp =>
            {
                ISnowpackProvider cached = new CachedSnowpackProvider(
                    sp.GetRequiredService<SyntheticSnowpackProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedSnowpackProvider>>());
                return new SnapshottingSnowpackProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingSnowpackProvider>>());
            });
        }

        // Gridded snow (seNorge) provider. Real impl when SeNorge:Enabled=true.
        services.Configure<SeNorgeOptions>(configuration.GetSection("SeNorge"));
        if (configuration.GetValue<bool>("SeNorge:Enabled"))
        {
            services.AddHttpClient(SeNorgeGriddedSnowProvider.HttpClientName, (sp, http) =>
            {
                var opts = sp.GetRequiredService<IOptions<SeNorgeOptions>>().Value;
                http.BaseAddress = new Uri(opts.BaseUrl);
                http.Timeout = TimeSpan.FromSeconds(8);
            })
            .AddStandardResilienceHandler(ConfigureUpstreamResilience);
            services.AddScoped<SeNorgeGriddedSnowProvider>();
            services.AddScoped<IGriddedSnowProvider>(sp =>
            {
                IGriddedSnowProvider cached = new CachedGriddedSnowProvider(
                    sp.GetRequiredService<SeNorgeGriddedSnowProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedGriddedSnowProvider>>());
                return new SnapshottingGriddedSnowProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingGriddedSnowProvider>>());
            });
            services.AddSingleton<IMetricExtractor>(new GriddedSnowMetricExtractor("senorge_gridded_snow"));
        }
        else
        {
            services.AddSingleton<SyntheticGriddedSnowProvider>();
            services.AddScoped<IGriddedSnowProvider>(sp =>
            {
                IGriddedSnowProvider cached = new CachedGriddedSnowProvider(
                    sp.GetRequiredService<SyntheticGriddedSnowProvider>(),
                    sp.GetRequiredService<IConditionsCache>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedGriddedSnowProvider>>());
                return new SnapshottingGriddedSnowProvider(
                    cached,
                    sp.GetRequiredService<IConditionsSnapshotStore>(),
                    sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingGriddedSnowProvider>>());
            });
            services.AddSingleton<IMetricExtractor>(new GriddedSnowMetricExtractor("synthetic_gridded_snow"));
        }

        // Persistent conditions snapshot store — orchestrators query this
        // for percentile/trend signals no upstream API exposes. The
        // metric-extractor registry stays empty for now; per-provider
        // extractors register themselves alongside their concrete
        // providers as those land.
        services.AddSingleton<IMetricExtractorRegistry, MetricExtractorRegistry>();
        services.AddScoped<IConditionsSnapshotStore, PgConditionsSnapshotStore>();

        // Own-data stores. Powers user-contributed observations + visits
        // that orchestrators fan in alongside external signals.
        services.AddScoped<IActivityObservationStore, PgActivityObservationStore>();
        services.AddScoped<IActivityVisitStore, PgActivityVisitStore>();

        // Score write-back. Every successful orchestrator run updates
        // the activity's summary row so the map's pin halos + the
        // recommendation endpoint see fresh scores without doing a
        // per-pin analysis fetch.
        services.AddScoped<IActivitySummaryScoreWriter, PgActivitySummaryScoreWriter>();

        // Turbidity (Sentinel-2 / Vannmiljø proxy) provider. Synthetic
        // only for now — the real product would pull recent
        // cloud-free Sentinel-2 pixels for the spot.
        services.AddSingleton<SyntheticTurbidityProvider>();
        services.AddScoped<ITurbidityProvider>(sp =>
        {
            ITurbidityProvider cached = new CachedTurbidityProvider(
                sp.GetRequiredService<SyntheticTurbidityProvider>(),
                sp.GetRequiredService<IConditionsCache>(),
                sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<CachedTurbidityProvider>>());
            return new SnapshottingTurbidityProvider(
                cached,
                sp.GetRequiredService<IConditionsSnapshotStore>(),
                sp.GetRequiredService<Microsoft.Extensions.Logging.ILogger<SnapshottingTurbidityProvider>>());
        });

        // Geo-context service. Real impl samples DEM along geometry,
        // derives ascent/descent/aspect/slope histograms, persists as
        // jsonb keyed on a hash of the geometry. Reads the geo_regions
        // table via IRegionPolygonStore to populate VarsomRegionId etc.
        services.AddScoped<IRegionPolygonStore, PgRegionPolygonStore>();
        services.AddScoped<IActivityGeoContextService, ActivityGeoContextService>();

        // GeoRegion seeder. Loads region polygons from configured
        // GeoJSON files. Off when no Sources are configured.
        services.Configure<GeoRegionSeederOptions>(configuration.GetSection("GeoRegionSeeder"));
        services.AddHostedService<GeoRegionSeederHostedService>();

        services.AddControllers().AddApplicationPart(typeof(ActivitySummariesController).Assembly);

        return services;
    }

    public static Task MigrateActivitiesSharedModuleAsync(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        => services.MigrateModuleDatabaseAsync<ActivitySummariesContext>(connectionString, Schema, cancellationToken);

    /// <summary>
    /// Shared resilience profile applied to every external conditions
    /// HTTP client. Wraps standard retry + circuit breaker around per-
    /// attempt timeouts so a flaky upstream (met.no, Varsom, NVE, ...)
    /// can't take the request thread or blow up downstream advisors.
    /// </summary>
    private static void ConfigureUpstreamResilience(HttpStandardResilienceOptions options)
    {
        // Generous per-attempt budget; the HttpClient itself caps the
        // total budget at 10s via Timeout (set per-named-client).
        options.AttemptTimeout.Timeout = TimeSpan.FromSeconds(4);
        options.TotalRequestTimeout.Timeout = TimeSpan.FromSeconds(12);

        options.Retry.MaxRetryAttempts = 2;
        options.Retry.BackoffType = Polly.DelayBackoffType.Exponential;
        options.Retry.UseJitter = true;

        // Default circuit breaker (50% failure ratio over a 30s window,
        // 30s break duration) is fine for our usage — these calls fan
        // out from each conditions request and we'd rather fail fast on
        // a degraded upstream than queue up retries.
    }
}
