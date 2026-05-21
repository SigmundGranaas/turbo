using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.Hiking.data.model;

namespace Turboapi.Activities.Hiking.data;

public class HikingContext : DbContext
{
    public DbSet<HikingActivityEntity> Activities { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public HikingContext(DbContextOptions<HikingContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("hiking");
        modelBuilder.MapProcessedEvents("hiking");

        modelBuilder.Entity<HikingActivityEntity>(entity =>
        {
            entity.ToTable("activities", "hiking");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");
            entity.Property(e => e.Route).HasColumnName("route").HasColumnType("geometry(LineString, 4326)").IsRequired();
            entity.Property(e => e.DistanceMeters).HasColumnName("distance_meters").IsRequired();
            entity.Property(e => e.AscentMeters).HasColumnName("ascent_meters").IsRequired();
            entity.Property(e => e.DescentMeters).HasColumnName("descent_meters").IsRequired();
            entity.Property(e => e.ElevationMinMeters).HasColumnName("elevation_min_meters").IsRequired();
            entity.Property(e => e.ElevationMaxMeters).HasColumnName("elevation_max_meters").IsRequired();
            entity.Property(e => e.Difficulty).HasColumnName("difficulty").IsRequired();
            entity.Property(e => e.Surface).HasColumnName("surface").IsRequired();
            entity.Property(e => e.Marking).HasColumnName("marking").IsRequired();
            entity.Property(e => e.EstimatedHours).HasColumnName("estimated_hours");
            entity.Property(e => e.HasWaterSources).HasColumnName("has_water_sources").IsRequired();
            entity.Property(e => e.HasShelter).HasColumnName("has_shelter").IsRequired();
            entity.Property(e => e.CreatedAt).HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP").ValueGeneratedOnAdd().IsRequired();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_hiking_activities_owner");
            entity.HasIndex(e => e.Route).HasDatabaseName("idx_hiking_activities_route").HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt }).HasDatabaseName("idx_hiking_activities_owner_updated_at");

            entity.HasMany(e => e.WaterSources)
                .WithOne()
                .HasForeignKey(w => w.ActivityId)
                .OnDelete(DeleteBehavior.Cascade);
        });

        modelBuilder.Entity<WaterSourceEntity>(entity =>
        {
            entity.ToTable("water_sources", "hiking");
            entity.HasKey(w => new { w.ActivityId, w.Ordinal });
            entity.Property(w => w.ActivityId).HasColumnName("activity_id");
            entity.Property(w => w.Ordinal).HasColumnName("ordinal");
            entity.Property(w => w.Lat).HasColumnName("lat").IsRequired();
            entity.Property(w => w.Lon).HasColumnName("lon").IsRequired();
            entity.Property(w => w.Kind).HasColumnName("kind").IsRequired();
            entity.Property(w => w.Notes).HasColumnName("notes");
        });
    }
}
