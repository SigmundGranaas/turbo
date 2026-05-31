using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.data.model;

namespace Turboapi.Activities.data;

public class ActivitySummariesContext : DbContext
{
    public DbSet<ActivitySummaryEntity> Summaries { get; set; } = null!;
    public DbSet<ConditionsCacheEntity> ConditionsCache { get; set; } = null!;
    public DbSet<ConditionsSnapshotEntity> ConditionsSnapshots { get; set; } = null!;
    public DbSet<ActivityObservationEntity> ActivityObservations { get; set; } = null!;
    public DbSet<ActivityVisitEntity> ActivityVisits { get; set; } = null!;
    public DbSet<ActivityGeoContextEntity> ActivityGeoContexts { get; set; } = null!;
    public DbSet<GeoRegionEntity> GeoRegions { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public ActivitySummariesContext(DbContextOptions<ActivitySummariesContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("activities");
        modelBuilder.MapProcessedEvents("activities");

        modelBuilder.Entity<ConditionsCacheEntity>(entity =>
        {
            entity.ToTable("conditions_cache", "activities");
            entity.HasKey(e => new { e.ProviderKey, e.GridCell, e.TimeBucket });
            entity.Property(e => e.ProviderKey).HasColumnName("provider_key").IsRequired();
            entity.Property(e => e.GridCell).HasColumnName("grid_cell").IsRequired();
            entity.Property(e => e.TimeBucket).HasColumnName("time_bucket").IsRequired();
            entity.Property(e => e.Payload).HasColumnName("payload").HasColumnType("bytea").IsRequired();
            entity.Property(e => e.FetchedAt).HasColumnName("fetched_at").IsRequired();
            entity.Property(e => e.ExpiresAt).HasColumnName("expires_at").IsRequired();
            entity.HasIndex(e => e.ExpiresAt).HasDatabaseName("idx_conditions_cache_expires_at");
        });

        modelBuilder.Entity<ActivitySummaryEntity>(entity =>
        {
            entity.ToTable("activity_summaries", "activities");

            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");

            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Kind).HasColumnName("kind").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();

            entity.Property(e => e.Geometry)
                .HasColumnName("geometry")
                .HasColumnType("geometry(Geometry, 4326)")
                .IsRequired();

            entity.Property(e => e.IconKey).HasColumnName("icon_key").IsRequired();
            entity.Property(e => e.ColorHex).HasColumnName("color_hex");

            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd()
                .IsRequired();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.Property(e => e.SummaryScore).HasColumnName("summary_score");
            entity.Property(e => e.SummaryScoreAt).HasColumnName("summary_score_at");
            entity.Property(e => e.TopDriverLabel).HasColumnName("top_driver_label");

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_activity_summaries_owner");
            entity.HasIndex(e => e.Geometry)
                .HasDatabaseName("idx_activity_summaries_geometry")
                .HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt })
                .HasDatabaseName("idx_activity_summaries_owner_updated_at");
            entity.HasIndex(e => e.Kind).HasDatabaseName("idx_activity_summaries_kind");
        });

        modelBuilder.Entity<ConditionsSnapshotEntity>(entity =>
        {
            entity.ToTable("conditions_snapshots", "activities");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id").ValueGeneratedOnAdd();
            entity.Property(e => e.ProviderKey).HasColumnName("provider_key").IsRequired();
            entity.Property(e => e.GridCell).HasColumnName("grid_cell").IsRequired();
            entity.Property(e => e.ObservedAt).HasColumnName("observed_at").IsRequired();
            entity.Property(e => e.FetchedAt).HasColumnName("fetched_at").IsRequired();
            entity.Property(e => e.Payload).HasColumnName("payload").HasColumnType("jsonb").IsRequired();
            entity.Property(e => e.PayloadSchemaVersion).HasColumnName("payload_schema_version").IsRequired();
            entity.HasIndex(e => new { e.ProviderKey, e.GridCell, e.ObservedAt })
                .HasDatabaseName("idx_conditions_snapshots_provider_grid_observed_at")
                .IsDescending(false, false, true);
        });

        modelBuilder.Entity<ActivityObservationEntity>(entity =>
        {
            entity.ToTable("activity_observations", "activities");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.ActivityId).HasColumnName("activity_id").IsRequired();
            entity.Property(e => e.UserId).HasColumnName("user_id").IsRequired();
            entity.Property(e => e.ObservedAt).HasColumnName("observed_at").IsRequired();
            entity.Property(e => e.Kind).HasColumnName("kind").IsRequired();
            entity.Property(e => e.Rating).HasColumnName("rating");
            entity.Property(e => e.Comment).HasColumnName("comment");
            entity.Property(e => e.KindPayload).HasColumnName("kind_payload").HasColumnType("jsonb").IsRequired();
            entity.Property(e => e.PhotoCount).HasColumnName("photo_count").HasDefaultValue((short)0);
            entity.Property(e => e.WatershedHrefId).HasColumnName("watershed_href_id");
            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.HasIndex(e => new { e.ActivityId, e.ObservedAt })
                .HasDatabaseName("idx_activity_observations_activity_observed_at")
                .IsDescending(false, true);
            entity.HasIndex(e => new { e.UserId, e.Kind, e.ObservedAt })
                .HasDatabaseName("idx_activity_observations_user_kind_observed_at")
                .IsDescending(false, false, true);
            entity.HasIndex(e => new { e.WatershedHrefId, e.ObservedAt })
                .HasDatabaseName("idx_activity_observations_watershed_observed_at")
                .IsDescending(false, true);
        });

        modelBuilder.Entity<ActivityVisitEntity>(entity =>
        {
            entity.ToTable("activity_visits", "activities");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.ActivityId).HasColumnName("activity_id").IsRequired();
            entity.Property(e => e.UserId).HasColumnName("user_id").IsRequired();
            entity.Property(e => e.VisitedAt).HasColumnName("visited_at").IsRequired();
            entity.Property(e => e.Source).HasColumnName("source").IsRequired();
            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.HasIndex(e => new { e.ActivityId, e.VisitedAt })
                .HasDatabaseName("idx_activity_visits_activity_visited_at")
                .IsDescending(false, true);
            entity.HasIndex(e => new { e.UserId, e.VisitedAt })
                .HasDatabaseName("idx_activity_visits_user_visited_at")
                .IsDescending(false, true);
        });

        modelBuilder.Entity<ActivityGeoContextEntity>(entity =>
        {
            entity.ToTable("activity_geo_contexts", "activities");
            entity.HasKey(e => e.ActivityId);
            entity.Property(e => e.ActivityId).HasColumnName("activity_id");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();
            entity.Property(e => e.GeomHash).HasColumnName("geom_hash").IsRequired();
            entity.Property(e => e.Payload).HasColumnName("payload").HasColumnType("jsonb").IsRequired();
            entity.Property(e => e.ComputedAt).HasColumnName("computed_at").IsRequired();
        });

        modelBuilder.Entity<GeoRegionEntity>(entity =>
        {
            entity.ToTable("geo_regions", "activities");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id").ValueGeneratedOnAdd();
            entity.Property(e => e.Source).HasColumnName("source").IsRequired();
            entity.Property(e => e.RegionId).HasColumnName("region_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Geometry)
                .HasColumnName("geometry")
                .HasColumnType("geometry(Geometry, 4326)")
                .IsRequired();
            entity.HasIndex(e => new { e.Source, e.RegionId })
                .HasDatabaseName("idx_geo_regions_source_region_id")
                .IsUnique();
            entity.HasIndex(e => e.Geometry)
                .HasDatabaseName("idx_geo_regions_geometry")
                .HasMethod("GIST");
        });
    }
}
