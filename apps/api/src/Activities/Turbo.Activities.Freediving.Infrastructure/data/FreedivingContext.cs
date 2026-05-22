using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.Freediving.data.model;

namespace Turboapi.Activities.Freediving.data;

public class FreedivingContext : DbContext
{
    public DbSet<FreedivingActivityEntity> Activities { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public FreedivingContext(DbContextOptions<FreedivingContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("freediving");
        modelBuilder.MapProcessedEvents("freediving");

        modelBuilder.Entity<FreedivingActivityEntity>(entity =>
        {
            entity.ToTable("activities", "freediving");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");
            entity.Property(e => e.Geometry).HasColumnName("geometry").HasColumnType("geometry(Point, 4326)").IsRequired();
            entity.Property(e => e.WaterBody).HasColumnName("water_body").IsRequired();
            entity.Property(e => e.BottomType).HasColumnName("bottom_type").IsRequired();
            entity.Property(e => e.MaxDepthMeters).HasColumnName("max_depth_meters").IsRequired();
            entity.Property(e => e.TypicalVisibilityMeters).HasColumnName("typical_visibility_meters");
            entity.Property(e => e.HarpoonAllowed).HasColumnName("harpoon_allowed").IsRequired();
            entity.Property(e => e.ShoreEntry).HasColumnName("shore_entry").IsRequired();
            entity.Property(e => e.AccessNotes).HasColumnName("access_notes");
            entity.Property(e => e.CreatedAt).HasColumnName("created_at")
                .HasDefaultValueSql("CURRENT_TIMESTAMP").ValueGeneratedOnAdd().IsRequired();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_freediving_activities_owner");
            entity.HasIndex(e => e.Geometry).HasDatabaseName("idx_freediving_activities_geometry").HasMethod("GIST");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt }).HasDatabaseName("idx_freediving_activities_owner_updated_at");

            entity.HasMany(e => e.TargetSpecies).WithOne()
                .HasForeignKey(t => t.ActivityId).OnDelete(DeleteBehavior.Cascade);
        });

        modelBuilder.Entity<TargetSpeciesEntity>(entity =>
        {
            entity.ToTable("target_species", "freediving");
            entity.HasKey(t => new { t.ActivityId, t.SpeciesCode });
            entity.Property(t => t.ActivityId).HasColumnName("activity_id");
            entity.Property(t => t.SpeciesCode).HasColumnName("species_code").IsRequired();
            entity.Property(t => t.Notes).HasColumnName("notes");
        });
    }
}
