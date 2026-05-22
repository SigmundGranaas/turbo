using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;
using Turboapi.Auth.Domain.Aggregates;

namespace Turboapi.Auth.Infrastructure.Persistence.Configuration
{
    public class AuthenticationMethodConfiguration : IEntityTypeConfiguration<AuthenticationMethod>
    {
        public void Configure(EntityTypeBuilder<AuthenticationMethod> builder)
        {
            builder.ToTable("authentication_methods");

            builder.HasKey(am => am.Id);
            builder.Property(am => am.Id)
                .HasColumnName("id")
                .ValueGeneratedNever();

            // Explicitly configure the AccountId property
            builder.Property(am => am.AccountId)
                .HasColumnName("account_id")
                .IsRequired();

            builder.Property(am => am.ProviderName)
                .HasColumnName("provider_name")
                .IsRequired()
                .HasMaxLength(50);

            builder.Property(am => am.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired();

            builder.Property(am => am.LastUsedAt)
                .HasColumnName("last_used_at");

            // Table-Per-Hierarchy (TPH) configuration
            builder.HasDiscriminator<string>("auth_type")
                .HasValue<PasswordAuthMethod>("Password")
                .HasValue<OAuthAuthMethod>("OAuth");
            
            builder.HasIndex(am => new { am.AccountId, am.ProviderName }).IsUnique();
        }
    }

    public class PasswordAuthMethodConfiguration : IEntityTypeConfiguration<PasswordAuthMethod>
    {
        public void Configure(EntityTypeBuilder<PasswordAuthMethod> builder)
        {
            builder.Property(pam => pam.PasswordHash)
                .HasColumnName("password_hash")
                .IsRequired(false)
                .HasMaxLength(512);
        }
    }

    public class OAuthAuthMethodConfiguration : IEntityTypeConfiguration<OAuthAuthMethod>
    {
        public void Configure(EntityTypeBuilder<OAuthAuthMethod> builder)
        {
            builder.Property(oam => oam.ExternalUserId)
                .HasColumnName("external_user_id")
                .IsRequired(false)
                .HasMaxLength(256);

            builder.Property(oam => oam.AccessToken)
                .HasColumnName("access_token")
                .HasMaxLength(2048);

            builder.Property(oam => oam.OAuthRefreshToken)
                .HasColumnName("oauth_refresh_token")
                .HasMaxLength(1024);

            builder.Property(oam => oam.TokenExpiry)
                .HasColumnName("token_expiry");

            builder.HasIndex(oam => new { oam.ProviderName, oam.ExternalUserId })
                   .IsUnique()
                   .HasFilter("\"external_user_id\" IS NOT NULL");
        }
    }
}