using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Geo.data.model;

namespace Turboapi.Geo.domain.query.model;

public class LocationReadContext : DbContext
{
    public DbSet<LocationEntity> Locations { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public LocationReadContext(DbContextOptions<LocationReadContext> options)
        : base(options)
    {
    }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("geo");
        modelBuilder.MapProcessedEvents("geo");

        modelBuilder.Entity<LocationEntity>(entity =>
        {
            entity.ToTable("locations_read");

            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id)
                .HasColumnName("id");

            entity.Property(e => e.OwnerId)
                .HasColumnName("owner_id")
                .IsRequired();

            entity.Property(e => e.Name)
                .HasColumnName("name");

            entity.Property(e => e.Description)
                .HasColumnName("description");

            entity.Property(e => e.Icon)
                .HasColumnName("icon");

            entity.Property(e => e.Geometry)
                .HasColumnName("geometry")
                .HasColumnType("geometry(Point, 4326)")
                .IsRequired();

            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();

            // Server-attested sync fields. UpdatedAt is set on every
            // projection write (create + update + delete); we don't lean on
            // EF's ValueGeneratedOnAddOrUpdate because the projection
            // explicitly stamps the value with the event's OccurredAt.
            entity.Property(e => e.UpdatedAt)
                .HasColumnName("updated_at")
                .IsRequired();

            entity.Property(e => e.DeletedAt)
                .HasColumnName("deleted_at")
                .IsRequired(false);

            entity.Property(e => e.Version)
                .HasColumnName("version")
                .IsRequired();

            entity.HasIndex(e => e.OwnerId)
                .HasDatabaseName("idx_locations_read_owner");

            entity.HasIndex(e => e.Geometry)
                .HasDatabaseName("idx_locations_read_geometry")
                .HasMethod("GIST");

            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt })
                .HasDatabaseName("idx_locations_read_owner_updated_at");
        });
    }
}
