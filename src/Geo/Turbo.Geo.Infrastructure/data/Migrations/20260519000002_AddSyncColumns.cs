using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Turboapi.Geo.data.Migrations
{
    /// <inheritdoc />
    public partial class AddSyncColumns : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            // Drop the old nullable updated_at so we can replace it with a
            // not-null one. EF doesn't support a clean ALTER from nullable
            // -> not-null + default through AlterColumn, so we drop & re-add.
            migrationBuilder.DropColumn(name: "updated_at", table: "locations_read");

            migrationBuilder.AddColumn<DateTime>(
                name: "updated_at",
                table: "locations_read",
                type: "timestamp with time zone",
                nullable: false,
                defaultValueSql: "CURRENT_TIMESTAMP");

            migrationBuilder.AddColumn<DateTime>(
                name: "deleted_at",
                table: "locations_read",
                type: "timestamp with time zone",
                nullable: true);

            migrationBuilder.AddColumn<long>(
                name: "version",
                table: "locations_read",
                type: "bigint",
                nullable: false,
                defaultValue: 1L);

            // Backfill: any row created before this migration has its
            // updated_at = created_at (server default already covers new
            // rows that arrive between the migration and the new code
            // shipping).
            migrationBuilder.Sql("UPDATE locations_read SET updated_at = created_at WHERE updated_at IS NULL OR updated_at = '0001-01-01';");

            migrationBuilder.CreateIndex(
                name: "idx_locations_read_owner_updated_at",
                table: "locations_read",
                columns: new[] { "owner_id", "updated_at" });
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropIndex(
                name: "idx_locations_read_owner_updated_at",
                table: "locations_read");

            migrationBuilder.DropColumn(name: "version", table: "locations_read");
            migrationBuilder.DropColumn(name: "deleted_at", table: "locations_read");
            migrationBuilder.DropColumn(name: "updated_at", table: "locations_read");

            migrationBuilder.AddColumn<DateTime>(
                name: "updated_at",
                table: "locations_read",
                type: "timestamp with time zone",
                nullable: true);
        }
    }
}
