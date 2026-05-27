using Microsoft.EntityFrameworkCore;
using Turbo.Outbox.Postgres;
using Turboapi.Sharing.data.model;

namespace Turboapi.Sharing.data;

public class SharingReadContext : DbContext
{
    public DbSet<ResourceEntity> Resources { get; set; } = null!;
    public DbSet<GrantEntity> Grants { get; set; } = null!;
    public DbSet<FriendshipEntity> Friendships { get; set; } = null!;
    public DbSet<GroupEntity> Groups { get; set; } = null!;
    public DbSet<GroupMemberEntity> GroupMembers { get; set; } = null!;
    public DbSet<ShareInviteEntity> ShareInvites { get; set; } = null!;
    public DbSet<OutboxRow> Outbox { get; set; } = null!;

    public SharingReadContext(DbContextOptions<SharingReadContext> options) : base(options) { }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.MapOutbox("sharing");
        modelBuilder.MapProcessedEvents("sharing");

        modelBuilder.Entity<ResourceEntity>(entity =>
        {
            entity.ToTable("resources", "sharing");
            entity.HasKey(e => e.Id);

            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.Type).HasColumnName("type").IsRequired();
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Visibility).HasColumnName("visibility").IsRequired();
            entity.Property(e => e.Version).HasColumnName("version").IsRequired();
            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at").IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();
            entity.Property(e => e.DeletedAt).HasColumnName("deleted_at");

            entity.HasIndex(e => new { e.OwnerId, e.Type })
                .HasDatabaseName("idx_resources_owner_type");
            entity.HasIndex(e => new { e.Type, e.Visibility })
                .HasDatabaseName("idx_resources_type_visibility");
            entity.HasIndex(e => e.UpdatedAt)
                .HasDatabaseName("idx_resources_updated_at");
        });

        modelBuilder.Entity<GrantEntity>(entity =>
        {
            entity.ToTable("grants", "sharing");
            entity.HasKey(e => new { e.ResourceId, e.SubjectType, e.SubjectId });

            entity.Property(e => e.ResourceId).HasColumnName("resource_id");
            entity.Property(e => e.SubjectType).HasColumnName("subject_type").IsRequired();
            entity.Property(e => e.SubjectId).HasColumnName("subject_id");
            entity.Property(e => e.Role).HasColumnName("role").IsRequired();
            entity.Property(e => e.GrantedBy).HasColumnName("granted_by").IsRequired();
            entity.Property(e => e.GrantedAt)
                .HasColumnName("granted_at").IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.Property(e => e.ExpiresAt).HasColumnName("expires_at");
            entity.Property(e => e.LinkToken).HasColumnName("link_token");

            entity.HasOne<ResourceEntity>()
                .WithMany()
                .HasForeignKey(g => g.ResourceId)
                .OnDelete(DeleteBehavior.Cascade);

            entity.HasIndex(e => new { e.SubjectType, e.SubjectId })
                .HasDatabaseName("idx_grants_subject");
            entity.HasIndex(e => e.LinkToken)
                .HasDatabaseName("idx_grants_link_token")
                .IsUnique()
                .HasFilter("link_token IS NOT NULL");
        });

        modelBuilder.Entity<FriendshipEntity>(entity =>
        {
            entity.ToTable("friendships", "sharing");
            entity.HasKey(e => new { e.LowerUserId, e.HigherUserId });

            entity.Property(e => e.LowerUserId).HasColumnName("lower_user_id");
            entity.Property(e => e.HigherUserId).HasColumnName("higher_user_id");
            entity.Property(e => e.InitiatorId).HasColumnName("initiator_id").IsRequired();
            entity.Property(e => e.Status).HasColumnName("status").IsRequired();
            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at").IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.Property(e => e.AcceptedAt).HasColumnName("accepted_at");

            entity.HasIndex(e => e.HigherUserId)
                .HasDatabaseName("idx_friendships_higher_user");
            entity.HasIndex(e => new { e.Status, e.LowerUserId })
                .HasDatabaseName("idx_friendships_status_lower");
        });

        modelBuilder.Entity<GroupEntity>(entity =>
        {
            entity.ToTable("groups", "sharing");
            entity.HasKey(e => e.Id);

            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.OwnerId).HasColumnName("owner_id").IsRequired();
            entity.Property(e => e.Name).HasColumnName("name").IsRequired();
            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at").IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.Property(e => e.UpdatedAt).HasColumnName("updated_at").IsRequired();

            entity.HasIndex(e => e.OwnerId)
                .HasDatabaseName("idx_groups_owner");
        });

        modelBuilder.Entity<GroupMemberEntity>(entity =>
        {
            entity.ToTable("group_members", "sharing");
            entity.HasKey(e => new { e.GroupId, e.UserId });

            entity.Property(e => e.GroupId).HasColumnName("group_id");
            entity.Property(e => e.UserId).HasColumnName("user_id");
            entity.Property(e => e.Role).HasColumnName("role").IsRequired();
            entity.Property(e => e.JoinedAt)
                .HasColumnName("joined_at").IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();

            entity.HasOne<GroupEntity>()
                .WithMany()
                .HasForeignKey(g => g.GroupId)
                .OnDelete(DeleteBehavior.Cascade);

            entity.HasIndex(e => e.UserId)
                .HasDatabaseName("idx_group_members_user");
        });

        modelBuilder.Entity<ShareInviteEntity>(entity =>
        {
            entity.ToTable("share_invites", "sharing");
            entity.HasKey(e => e.Id);

            entity.Property(e => e.Id).HasColumnName("id");
            entity.Property(e => e.InviterId).HasColumnName("inviter_id").IsRequired();
            entity.Property(e => e.InviteeEmail).HasColumnName("invitee_email").IsRequired();
            entity.Property(e => e.ResourceId).HasColumnName("resource_id");
            entity.Property(e => e.Role).HasColumnName("role");
            entity.Property(e => e.CreatedAt)
                .HasColumnName("created_at").IsRequired()
                .HasDefaultValueSql("CURRENT_TIMESTAMP")
                .ValueGeneratedOnAdd();
            entity.Property(e => e.ExpiresAt).HasColumnName("expires_at");
            entity.Property(e => e.RedeemedAt).HasColumnName("redeemed_at");
            entity.Property(e => e.RedeemedByUserId).HasColumnName("redeemed_by_user_id");

            entity.HasIndex(e => new { e.InviteeEmail, e.RedeemedAt })
                .HasDatabaseName("idx_share_invites_email_pending");
        });
    }
}
