using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.data.model;

namespace Turboapi.Activities.data;

public class ActivitySummariesContext : DbContext
{
    public DbSet<ActivitySummaryEntity> Summaries { get; set; } = null!;
    public DbSet<ConditionsCacheEntity> ConditionsCache { get; set; } = null!;
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

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_activity_summaries_owner");
            entity.HasIndex(e => e.Geometry)
                .HasDatabaseName("idx_activity_summaries_geometry")
                .HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt })
                .HasDatabaseName("idx_activity_summaries_owner_updated_at");
            entity.HasIndex(e => e.Kind).HasDatabaseName("idx_activity_summaries_kind");
        });
    }
}
