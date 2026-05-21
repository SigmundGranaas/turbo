using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.XcSki.data.model;

namespace Turboapi.Activities.XcSki.data;

public class XcSkiContext : DbContext
{
    public DbSet<XcSkiActivityEntity> Activities { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public XcSkiContext(DbContextOptions<XcSkiContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("xc_ski");
        modelBuilder.MapProcessedEvents("xc_ski");

        modelBuilder.Entity<XcSkiActivityEntity>(entity =>
        {
            entity.ToTable("activities", "xc_ski");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");
            entity.Property(e => e.Route).HasColumnName("route").HasColumnType("geometry(LineString, 4326)").IsRequired();
            entity.Property(e => e.DistanceMeters).HasColumnName("distance_meters").IsRequired();
            entity.Property(e => e.AscentMeters).HasColumnName("ascent_meters").IsRequired();
            entity.Property(e => e.DescentMeters).HasColumnName("descent_meters").IsRequired();
            entity.Property(e => e.Technique).HasColumnName("technique").IsRequired();
            entity.Property(e => e.GroomingStatus).HasColumnName("grooming_status").IsRequired();
            entity.Property(e => e.IsLit).HasColumnName("is_lit").IsRequired();
            entity.Property(e => e.RequiresSeasonPass).HasColumnName("requires_season_pass").IsRequired();
            entity.Property(e => e.GroomingFeedKey).HasColumnName("grooming_feed_key");
            entity.Property(e => e.CreatedAt).HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP").ValueGeneratedOnAdd().IsRequired();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_xc_ski_activities_owner");
            entity.HasIndex(e => e.Route).HasDatabaseName("idx_xc_ski_activities_route").HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt }).HasDatabaseName("idx_xc_ski_activities_owner_updated_at");
        });
    }
}
