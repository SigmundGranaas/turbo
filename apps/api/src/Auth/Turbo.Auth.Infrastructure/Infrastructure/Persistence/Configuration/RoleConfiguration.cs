using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;
using Turboapi.Auth.Domain.Aggregates;

namespace Turboapi.Auth.Infrastructure.Persistence.Configuration
{
    public class RoleConfiguration : IEntityTypeConfiguration<Role>
    {
        public void Configure(EntityTypeBuilder<Role> builder)
        {
            builder.ToTable("roles");

            builder.HasKey(r => r.Id);
            builder.Property(r => r.Id)
                .HasColumnName("id")
                .ValueGeneratedNever();

            // Explicitly configure the AccountId property
            builder.Property(r => r.AccountId)
                .HasColumnName("account_id")
                .IsRequired();

            builder.Property(r => r.Name)
                .HasColumnName("name")
                .IsRequired()
                .HasMaxLength(50);

            builder.Property(r => r.CreatedAt)
                .HasColumnName("created_at")
                .IsRequired();
            
            builder.HasIndex(r => new { r.AccountId, r.Name }).IsUnique();
        }
    }
}