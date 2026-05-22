using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using Turboapi.Activities.data;

#nullable disable

namespace Turboapi.Activities.data.Migrations
{
    /// <summary>
    /// Adds the conditions_cache table. Composite PK on
    /// (provider_key, grid_cell, time_bucket) — providers snap their
    /// inputs to that resolution before lookup, so two nearby points in
    /// the same hour share one upstream call.
    /// </summary>
    [DbContext(typeof(ActivitySummariesContext))]
    [Migration("20260520000001_AddConditionsCache")]
    public partial class AddConditionsCache : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.CreateTable(
                name: "conditions_cache",
                schema: "activities",
                columns: table => new
                {
                    provider_key = table.Column<string>(type: "text", nullable: false),
                    grid_cell = table.Column<string>(type: "text", nullable: false),
                    time_bucket = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    payload = table.Column<byte[]>(type: "bytea", nullable: false),
                    fetched_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    expires_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_conditions_cache", x => new { x.provider_key, x.grid_cell, x.time_bucket });
                });

            migrationBuilder.CreateIndex(
                name: "idx_conditions_cache_expires_at",
                schema: "activities",
                table: "conditions_cache",
                column: "expires_at");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "conditions_cache", schema: "activities");
        }
    }
}
