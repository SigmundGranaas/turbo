using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;
using Turboapi.Auth.Domain.Aggregates;

namespace Turboapi.Auth.Infrastructure.Persistence.Configuration
{
    public class AccountConfiguration : IEntityTypeConfiguration<Account>
    {
        public void Configure(EntityTypeBuilder<Account> builder)
        {
            builder.ToTable("accounts");

            builder.HasKey(a => a.Id);
            builder.Property(a => a.Id)
                .HasColumnName("id")
                .ValueGeneratedNever();

            builder.Property(a => a.Email)
                .HasColumnName("email")
                .IsRequired()
                .HasMaxLength(320); // RFC 5321 max

            builder.HasIndex(a => a.Email).IsUnique();

            builder.Property(a => a.DisplayName)
                .HasColumnName("display_name")
                .HasMaxLength(Domain.Aggregates.Account.MaxDisplayNameLength);

            builder.Property(a => a.IsActive)
                .HasColumnName("is_active")
                .IsRequired()
                .HasDefaultValue(true); // New mapping

            builder.Property(a => a.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired();

            builder.Property(a => a.LastLoginAt)
                .HasColumnName("last_login_at");

            // Configure relationships - Let child entities handle their own FK configuration
            builder.HasMany(a => a.Roles)
                .WithOne()
                .HasForeignKey("AccountId") // Use property name, not column name
                .IsRequired()
                .OnDelete(DeleteBehavior.Cascade);

            builder.HasMany(a => a.AuthenticationMethods)
                .WithOne()
                .HasForeignKey("AccountId") // Use property name, not column name
                .IsRequired()
                .OnDelete(DeleteBehavior.Cascade);
            
            builder.HasMany(a => a.RefreshTokens)
                .WithOne()
                .HasForeignKey("AccountId") // Use property name, not column name
                .IsRequired()
                .OnDelete(DeleteBehavior.Cascade);

            // Ignore DomainEvents collection from being mapped to the database
            builder.Ignore(a => a.DomainEvents);
        }
    }
}