using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.Fishing.data.model;

namespace Turboapi.Activities.Fishing.data;

public class FishingContext : DbContext
{
    public DbSet<FishingActivityEntity> Activities { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public FishingContext(DbContextOptions<FishingContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("fishing");
        modelBuilder.MapProcessedEvents("fishing");

        modelBuilder.Entity<FishingActivityEntity>(entity =>
        {
            entity.ToTable("activities", "fishing");

            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");

            entity.Property(e => e.Geometry)
                .HasColumnName("geometry")
                .HasColumnType("geometry(Point, 4326)")
                .IsRequired();

            entity.Property(e => e.WaterKind).HasColumnName("water_kind").IsRequired();
            entity.Property(e => e.ShoreOrBoat).HasColumnName("shore_or_boat").IsRequired();
            entity.Property(e => e.AccessNotes).HasColumnName("access_notes");

            entity.Property(e => e.PreferredPressureMinHpa).HasColumnName("preferred_pressure_min_hpa");
            entity.Property(e => e.PreferredPressureMaxHpa).HasColumnName("preferred_pressure_max_hpa");
            entity.Property(e => e.PreferredWindMaxMs).HasColumnName("preferred_wind_max_ms");

            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd()
                .IsRequired();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_fishing_activities_owner");
            entity.HasIndex(e => e.Geometry)
                .HasDatabaseName("idx_fishing_activities_geometry")
                .HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt })
                .HasDatabaseName("idx_fishing_activities_owner_updated_at");

            entity.HasMany(e => e.TargetSpecies)
                .WithOne()
                .HasForeignKey(t => t.ActivityId)
                .OnDelete(DeleteBehavior.Cascade);

            entity.HasMany(e => e.DepthSamples)
                .WithOne()
                .HasForeignKey(d => d.ActivityId)
                .OnDelete(DeleteBehavior.Cascade);
        });

        modelBuilder.Entity<TargetSpeciesEntity>(entity =>
        {
            entity.ToTable("target_species", "fishing");
            entity.HasKey(t => new { t.ActivityId, t.SpeciesCode });
            entity.Property(t => t.ActivityId).HasColumnName("activity_id");
            entity.Property(t => t.SpeciesCode).HasColumnName("species_code").IsRequired();
            entity.Property(t => t.Notes).HasColumnName("notes");
        });

        modelBuilder.Entity<DepthSampleEntity>(entity =>
        {
            entity.ToTable("depth_samples", "fishing");
            entity.HasKey(d => new { d.ActivityId, d.Ordinal });
            entity.Property(d => d.ActivityId).HasColumnName("activity_id");
            entity.Property(d => d.Ordinal).HasColumnName("ordinal");
            entity.Property(d => d.Lat).HasColumnName("lat").IsRequired();
            entity.Property(d => d.Lon).HasColumnName("lon").IsRequired();
            entity.Property(d => d.DepthMeters).HasColumnName("depth_meters").IsRequired();
        });
    }
}
