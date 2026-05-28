using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;
using Turboapi.Auth.Domain.Notifications;

namespace Turboapi.Auth.Infrastructure.Persistence.Configuration
{
    public class DeviceTokenConfiguration : IEntityTypeConfiguration<DeviceToken>
    {
        public void Configure(EntityTypeBuilder<DeviceToken> builder)
        {
            builder.ToTable("device_tokens");

            builder.HasKey(d => d.Token);
            builder.Property(d => d.Token)
                .HasColumnName("token")
                .HasMaxLength(512) // FCM/APNs tokens are well under this; keeps the PK b-tree index within Postgres limits
                .ValueGeneratedNever();

            builder.Property(d => d.AccountId)
                .HasColumnName("account_id")
                .IsRequired();

            builder.Property(d => d.Platform)
                .HasColumnName("platform")
                .HasMaxLength(16)
                .IsRequired();

            builder.Property(d => d.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired();

            builder.Property(d => d.LastSeenAt)
                .HasColumnName("last_seen_at")
                .IsRequired();

            builder.HasIndex(d => d.AccountId);
        }
    }
}
