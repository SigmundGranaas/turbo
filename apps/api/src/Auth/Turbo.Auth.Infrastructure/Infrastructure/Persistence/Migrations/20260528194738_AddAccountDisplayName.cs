using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Turboapi.Auth.Infrastructure.Persistence.Migrations
{
    /// <inheritdoc />
    public partial class AddAccountDisplayName : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.AddColumn<string>(
                name: "display_name",
                table: "accounts",
                type: "character varying(64)",
                maxLength: 64,
                nullable: true);
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropColumn(
                name: "display_name",
                table: "accounts");
        }
    }
}
