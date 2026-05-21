using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.Packrafting.data.model;

namespace Turboapi.Activities.Packrafting.data;

public class PackraftingContext : DbContext
{
    public DbSet<PackraftingActivityEntity> Activities { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public PackraftingContext(DbContextOptions<PackraftingContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("packrafting");
        modelBuilder.MapProcessedEvents("packrafting");

        modelBuilder.Entity<PackraftingActivityEntity>(entity =>
        {
            entity.ToTable("activities", "packrafting");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");
            entity.Property(e => e.Route).HasColumnName("route").HasColumnType("geometry(LineString, 4326)").IsRequired();
            entity.Property(e => e.DistanceMeters).HasColumnName("distance_meters").IsRequired();
            entity.Property(e => e.PaddleDistanceMeters).HasColumnName("paddle_distance_meters").IsRequired();
            entity.Property(e => e.PortageDistanceMeters).HasColumnName("portage_distance_meters").IsRequired();
            entity.Property(e => e.MaxGrade).HasColumnName("max_grade").IsRequired();
            entity.Property(e => e.TypicalGrade).HasColumnName("typical_grade").IsRequired();
            entity.Property(e => e.PutInLat).HasColumnName("put_in_lat").IsRequired();
            entity.Property(e => e.PutInLon).HasColumnName("put_in_lon").IsRequired();
            entity.Property(e => e.TakeOutLat).HasColumnName("take_out_lat").IsRequired();
            entity.Property(e => e.TakeOutLon).HasColumnName("take_out_lon").IsRequired();
            entity.Property(e => e.NveStationCode).HasColumnName("nve_station_code");
            entity.Property(e => e.MinFlowCumecs).HasColumnName("min_flow_cumecs");
            entity.Property(e => e.MaxFlowCumecs).HasColumnName("max_flow_cumecs");
            entity.Property(e => e.CreatedAt).HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP").ValueGeneratedOnAdd().IsRequired();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_packrafting_activities_owner");
            entity.HasIndex(e => e.Route).HasDatabaseName("idx_packrafting_activities_route").HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt }).HasDatabaseName("idx_packrafting_activities_owner_updated_at");

            entity.HasMany(e => e.Segments).WithOne()
                .HasForeignKey(s => s.ActivityId).OnDelete(DeleteBehavior.Cascade);
        });

        modelBuilder.Entity<RouteSegmentEntity>(entity =>
        {
            entity.ToTable("segments", "packrafting");
            entity.HasKey(s => new { s.ActivityId, s.Ordinal });
            entity.Property(s => s.ActivityId).HasColumnName("activity_id");
            entity.Property(s => s.Ordinal).HasColumnName("ordinal");
            entity.Property(s => s.Kind).HasColumnName("kind").IsRequired();
            entity.Property(s => s.Grade).HasColumnName("grade");
            entity.Property(s => s.DistanceMeters).HasColumnName("distance_meters").IsRequired();
            entity.Property(s => s.Geometry).HasColumnName("geometry").HasColumnType("geometry(LineString, 4326)").IsRequired();
            entity.Property(s => s.Notes).HasColumnName("notes");
        });
    }
}
