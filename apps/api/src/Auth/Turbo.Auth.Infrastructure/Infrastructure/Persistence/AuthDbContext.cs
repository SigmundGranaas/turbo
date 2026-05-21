using Microsoft.EntityFrameworkCore;
using System.Reflection;
using Turbo.Outbox.Postgres;
using Turboapi.Auth.Domain.Aggregates;

namespace Turboapi.Auth.Infrastructure.Persistence
{
    public class AuthDbContext : DbContext
    {
        public AuthDbContext(DbContextOptions<AuthDbContext> options) : base(options)
        {
        }

        public DbSet<Account> Accounts { get; set; } = null!;
        public DbSet<AuthenticationMethod> AuthenticationMethods { get; set; } = null!;
        public DbSet<PasswordAuthMethod> PasswordAuthMethods { get; set; } = null!;
        public DbSet<OAuthAuthMethod> OAuthAuthMethods { get; set; } = null!;
        public DbSet<RefreshToken> RefreshTokens { get; set; } = null!;
        public DbSet<Role> Roles { get; set; } = null!;
        public DbSet<OutboxRow> Outbox { get; set; } = null!;

        protected override void OnModelCreating(ModelBuilder modelBuilder)
        {
            base.OnModelCreating(modelBuilder);

            // Apply all IEntityTypeConfiguration classes from the current assembly
            modelBuilder.ApplyConfigurationsFromAssembly(Assembly.GetExecutingAssembly());

            modelBuilder.MapOutbox("auth");
            modelBuilder.MapProcessedEvents("auth");
        }
    }
}