using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;
using Turboapi.Auth.Domain.Aggregates;

namespace Turboapi.Auth.Infrastructure.Persistence.Configuration
{
    public class RefreshTokenConfiguration : IEntityTypeConfiguration<RefreshToken>
    {
        public void Configure(EntityTypeBuilder<RefreshToken> builder)
        {
            builder.ToTable("refresh_tokens");

            builder.HasKey(rt => rt.Id);
            builder.Property(rt => rt.Id)
                .HasColumnName("id")
                .ValueGeneratedNever();

            // Explicitly configure the AccountId property and its column mapping
            builder.Property(rt => rt.AccountId)
                .HasColumnName("account_id")
                .IsRequired();

            builder.Property(rt => rt.Token)
                .HasColumnName("token")
                .IsRequired()
                .HasMaxLength(256);

            builder.HasIndex(rt => rt.Token).IsUnique();

            builder.Property(rt => rt.ExpiresAt)
                .HasColumnName("expires_at")
                .IsRequired();

            builder.Property(rt => rt.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired();

            builder.Property(rt => rt.IsRevoked)
                .HasColumnName("is_revoked")
                .IsRequired();

            builder.Property(rt => rt.RevokedAt)
                .HasColumnName("revoked_at");

            builder.Property(rt => rt.RevokedReason)
                .HasColumnName("revoked_reason")
                .HasMaxLength(256);
        }
    }
}