using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Turboapi.Auth.Infrastructure.Persistence.Migrations
{
    /// <summary>
    /// Seed the <c>curator</c> and <c>admin</c> roles on the developer
    /// account. Used by the tileserver's admin panel — the Rust
    /// <c>turbo-tiles-auth::RequireRole&lt;Curator&gt;</c> extractor
    /// reads the role claim issued by <c>JwtService</c> and gates
    /// /admin/* on its presence.
    ///
    /// Idempotent: if the account doesn't exist yet (clean install)
    /// the insert is a no-op; if the roles are already there the
    /// NOT EXISTS guard prevents duplicates.
    /// </summary>
    public partial class SeedAdminRoles : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.Sql(@"
                INSERT INTO roles (id, account_id, name, created_at)
                SELECT gen_random_uuid(), a.id, r.role_name, now() AT TIME ZONE 'utc'
                FROM accounts a
                CROSS JOIN (VALUES ('curator'), ('admin')) AS r(role_name)
                WHERE a.email = 'sigmundsgranaas@gmail.com'
                  AND NOT EXISTS (
                      SELECT 1 FROM roles existing
                      WHERE existing.account_id = a.id
                        AND existing.name = r.role_name
                  );
            ");
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.Sql(@"
                DELETE FROM roles
                WHERE name IN ('curator', 'admin')
                  AND account_id IN (
                      SELECT id FROM accounts
                      WHERE email = 'sigmundsgranaas@gmail.com'
                  );
            ");
        }
    }
}
