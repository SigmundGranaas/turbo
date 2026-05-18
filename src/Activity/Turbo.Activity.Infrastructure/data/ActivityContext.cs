using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Activity.domain.query;

namespace Turboapi.Activity.data;

public class ActivityContext : DbContext
{
    public DbSet<ActivityQueryDto> Activities { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public ActivityContext(DbContextOptions<ActivityContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.Entity<ActivityQueryDto>(entity =>
        {
            entity.ToTable("activity_query");
            entity.HasKey(e => e.Position);

            entity.Property(e => e.Position).HasColumnName("position");
            entity.Property(e => e.ActivityId).HasColumnName("activity_id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id");
            entity.Property(e => e.Name).HasColumnName("name").IsRequired().HasMaxLength(255);
            // V1 had description TEXT (nullable) and icon VARCHAR(255) (nullable).
            // The C# property type is non-nullable, but the projection path
            // writes null when the caller doesn't supply a value.
            entity.Property(e => e.Description).HasColumnName("description").IsRequired(false);
            entity.Property(e => e.Icon).HasColumnName("icon").IsRequired(false).HasMaxLength(255);
            entity.HasIndex(e => e.ActivityId).HasDatabaseName("idx_activity_query_activity_id");
            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_activity_query_owner_id");
        });

        modelBuilder.MapOutbox("activity");
        modelBuilder.MapProcessedEvents("activity");
    }
}