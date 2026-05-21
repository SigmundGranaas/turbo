using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Tracks.data.model;

namespace Turboapi.Tracks.data;

public class TrackReadContext : DbContext
{
    public DbSet<TrackEntity> Tracks { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public TrackReadContext(DbContextOptions<TrackReadContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("tracks");
        modelBuilder.MapProcessedEvents("tracks");

        modelBuilder.Entity<TrackEntity>(entity =>
        {
            entity.ToTable("tracks_read");

            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");

            entity.Property(e => e.OwnerId)
                .HasColumnName("owner_id")
                .IsRequired();

            entity.Property(e => e.Geometry)
                .HasColumnName("geometry")
                .HasColumnType("geometry(LineString, 4326)")
                .IsRequired();

            entity.Property(e => e.Elevations)
                .HasColumnName("elevations")
                .HasColumnType("double precision[]");

            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");
            entity.Property(e => e.ColorHex).HasColumnName("color_hex");
            entity.Property(e => e.IconKey).HasColumnName("icon_key");
            entity.Property(e => e.LineStyleKey).HasColumnName("line_style_key");
            entity.Property(e => e.Smoothing).HasColumnName("smoothing").IsRequired();

            entity.Property(e => e.DistanceMeters).HasColumnName("distance_meters").IsRequired();
            entity.Property(e => e.AscentMeters).HasColumnName("ascent_meters");
            entity.Property(e => e.DescentMeters).HasColumnName("descent_meters");
            entity.Property(e => e.MovingTimeSeconds).HasColumnName("moving_time_seconds");
            entity.Property(e => e.RecordedAt).HasColumnName("recorded_at");

            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.Property(e => e.UpdatedAt)
                .HasColumnName("updated_at")
                .IsRequired();
            entity.Property(e => e.DeletedAt)
                .HasColumnName("deleted_at")
                .IsRequired(false);
            entity.Property(e => e.Version)
                .HasColumnName("version")
                .IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_tracks_read_owner");
            entity.HasIndex(e => e.Geometry)
                .HasDatabaseName("idx_tracks_read_geometry")
                .HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt })
                .HasDatabaseName("idx_tracks_read_owner_updated_at");
        });
    }
}
