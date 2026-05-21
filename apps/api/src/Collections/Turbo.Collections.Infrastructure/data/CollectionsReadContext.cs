using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Collections.data.model;

namespace Turboapi.Collections.data;

public class CollectionsReadContext : DbContext
{
    public DbSet<CollectionEntity> Collections { get; set; } = null!;
    public DbSet<CollectionItemEntity> CollectionItems { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public CollectionsReadContext(DbContextOptions<CollectionsReadContext> options)
        : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("collections");
        modelBuilder.MapProcessedEvents("collections");

        modelBuilder.Entity<CollectionEntity>(entity =>
        {
            entity.ToTable("collections_read");

            entity.HasKey(e => e.Id);
            entity.Property(e => e.Id).HasColumnName("id");

            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.Description).HasColumnName("description");
            entity.Property(e => e.ColorHex).HasColumnName("color_hex");
            entity.Property(e => e.IconKey).HasColumnName("icon_key");
            entity.Property(e => e.SortOrder).HasColumnName("sort_order").IsRequired();
            entity.Property(e => e.SavedFilterJson)
                .HasColumnName("saved_filter")
                .HasColumnType("jsonb");

            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at").IsRequired(false);
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();

            entity.HasIndex(e => e.OwnerId).HasDatabaseName("idx_collections_read_owner");
            entity.HasIndex(e => new { e.OwnerId, e.UpdatedAt })
                .HasDatabaseName("idx_collections_read_owner_updated_at");

            entity.HasMany(e => e.Items)
                .WithOne()
                .HasForeignKey(i => i.CollectionId)
                .OnDelete(DeleteBehavior.Cascade);
        });

        modelBuilder.Entity<CollectionItemEntity>(entity =>
        {
            entity.ToTable("collection_items_read");

            entity.HasKey(e => new { e.CollectionId, e.ItemType, e.ItemUuid });
            entity.Property(e => e.CollectionId).HasColumnName("collection_id");
            entity.Property(e => e.ItemType).HasColumnName("item_type");
            entity.Property(e => e.ItemUuid).HasColumnName("item_uuid");
            entity.Property(e => e.AddedAt)
                .HasColumnName("added_at")
                .IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();

            entity.HasIndex(e => new { e.ItemType, e.ItemUuid })
                .HasDatabaseName("idx_collection_items_read_item");
        });
    }
}
