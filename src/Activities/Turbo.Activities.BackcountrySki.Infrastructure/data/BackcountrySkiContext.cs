using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.BackcountrySki.data.model;

namespace Turboapi.Activities.BackcountrySki.data;

public class BackcountrySkiContext : DbContext
{
    public DbSet<BackcountrySkiActivityEntity> Activities { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public BackcountrySkiContext(DbContextOptions<BackcountrySkiContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("backcountry_ski");
        modelBuilder.MapProcessedEvents("backcountry_ski");

        modelBuilder.Entity<BackcountrySkiActivityEntity>(entity =>
        {
            entity.ToTable("activities", "backcountry_ski");

            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");

            entity.Property(e => e.Route)
                .HasColumnName("route")
                .HasColumnType("geometry(LineString, 4326)")
                .IsRequired();

            entity.Property(e => e.AscentMeters).HasColumnName("ascent_meters").IsRequired();
            entity.Property(e => e.DescentMeters).HasColumnName("descent_meters").IsRequired();
            entity.Property(e => e.DistanceMeters).HasColumnName("distance_meters").IsRequired();
            entity.Property(e => e.ElevationMinMeters).HasColumnName("elevation_min_meters").IsRequired();
            entity.Property(e => e.ElevationMaxMeters).HasColumnName("elevation_max_meters").IsRequired();

            entity.Property(e => e.AtesRating).HasColumnName("ates_rating").IsRequired();
            entity.Property(e => e.DominantAspect).HasColumnName("dominant_aspect");
            entity.Property(e => e.VarsomRegionId).HasColumnName("varsom_region_id");
            entity.Property(e => e.PreferredAvalancheMaxLevel).HasColumnName("preferred_avalanche_max_level");

            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd()
                .IsRequired();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_backcountry_ski_activities_owner");
            entity.HasIndex(e => e.Route)
                .HasDatabaseName("idx_backcountry_ski_activities_route")
                .HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt })
                .HasDatabaseName("idx_backcountry_ski_activities_owner_updated_at");

            entity.HasMany(e => e.AspectMix)
                .WithOne()
                .HasForeignKey(a => a.ActivityId)
                .OnDelete(DeleteBehavior.Cascade);

            entity.HasMany(e => e.Legs)
                .WithOne()
                .HasForeignKey(l => l.ActivityId)
                .OnDelete(DeleteBehavior.Cascade);
        });

        modelBuilder.Entity<AspectShareEntity>(entity =>
        {
            entity.ToTable("aspect_mix", "backcountry_ski");
            entity.HasKey(a => new { a.ActivityId, a.Aspect });
            entity.Property(a => a.ActivityId).HasColumnName("activity_id");
            entity.Property(a => a.Aspect).HasColumnName("aspect");
            entity.Property(a => a.Fraction).HasColumnName("fraction").IsRequired();
        });

        modelBuilder.Entity<RouteLegEntity>(entity =>
        {
            entity.ToTable("legs", "backcountry_ski");
            entity.HasKey(l => new { l.ActivityId, l.Ordinal });
            entity.Property(l => l.ActivityId).HasColumnName("activity_id");
            entity.Property(l => l.Ordinal).HasColumnName("ordinal");
            entity.Property(l => l.LegKind).HasColumnName("leg_kind").IsRequired();
            entity.Property(l => l.StartElevationMeters).HasColumnName("start_elevation_meters").IsRequired();
            entity.Property(l => l.EndElevationMeters).HasColumnName("end_elevation_meters").IsRequired();
            entity.Property(l => l.Geometry)
                .HasColumnName("geometry")
                .HasColumnType("geometry(LineString, 4326)")
                .IsRequired();
        });
    }
}
