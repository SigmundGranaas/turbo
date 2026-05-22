using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;
using Turboapi.Activities.Freediving.data;

#nullable disable

namespace Turboapi.Activities.Freediving.data.Migrations
{
    [DbContext(typeof(FreedivingContext))]
    [Migration("20260519000007_InitialCreate")]
    public partial class InitialCreate : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(name: "freediving");
            migrationBuilder.AlterDatabase().Annotation("Npgsql:PostgresExtension:postgis", ",,");

            migrationBuilder.CreateTable(
                name: "activities",
                schema: "freediving",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    description = table.Column<string>(type: "text", nullable: true),
                    geometry = table.Column<Point>(type: "geometry(Point, 4326)", nullable: false),
                    water_body = table.Column<short>(type: "smallint", nullable: false),
                    bottom_type = table.Column<short>(type: "smallint", nullable: false),
                    max_depth_meters = table.Column<float>(type: "real", nullable: false),
                    typical_visibility_meters = table.Column<float>(type: "real", nullable: true),
                    harpoon_allowed = table.Column<bool>(type: "boolean", nullable: false),
                    shore_entry = table.Column<bool>(type: "boolean", nullable: false),
                    access_notes = table.Column<string>(type: "text", nullable: true),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    version = table.Column<long>(type: "bigint", nullable: false)
                },
                constraints: table => { table.PrimaryKey("PK_freediving_activities", x => x.id); });

            migrationBuilder.CreateTable(
                name: "target_species",
                schema: "freediving",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    species_code = table.Column<string>(type: "text", nullable: false),
                    notes = table.Column<string>(type: "text", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_freediving_target_species", x => new { x.activity_id, x.species_code });
                    table.ForeignKey("FK_target_species_activities", x => x.activity_id, "activities", "id", principalSchema: "freediving", onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "freediving",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    aggregate_id = table.Column<Guid>(type: "uuid", nullable: false),
                    event_type = table.Column<string>(type: "text", nullable: false),
                    source = table.Column<string>(type: "text", nullable: false),
                    data_content_type = table.Column<string>(type: "text", nullable: false),
                    payload_json = table.Column<string>(type: "jsonb", nullable: false),
                    headers_json = table.Column<string>(type: "jsonb", nullable: false),
                    occurred_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    position = table.Column<long>(type: "bigint", nullable: false)
                        .Annotation("Npgsql:ValueGenerationStrategy", NpgsqlValueGenerationStrategy.IdentityByDefaultColumn),
                    dispatched_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    attempts = table.Column<int>(type: "integer", nullable: false),
                    last_error = table.Column<string>(type: "text", nullable: true)
                },
                constraints: table => { table.PrimaryKey("PK_outbox", x => x.id); });

            migrationBuilder.CreateTable(
                name: "processed_events",
                schema: "freediving",
                columns: table => new
                {
                    event_id = table.Column<Guid>(type: "uuid", nullable: false),
                    processed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table => { table.PrimaryKey("PK_processed_events", x => x.event_id); });

            migrationBuilder.CreateIndex("idx_freediving_activities_geometry", "activities", "geometry", "freediving").Annotation("Npgsql:IndexMethod", "GIST");
            migrationBuilder.CreateIndex("idx_freediving_activities_owner", "activities", "owner_id", "freediving");
            migrationBuilder.CreateIndex("idx_freediving_activities_owner_updated_at", "activities", new[] { "owner_id", "updated_at" }, "freediving");
            migrationBuilder.CreateIndex("outbox_aggregate", "outbox", new[] { "aggregate_id", "position" }, "freediving");
            migrationBuilder.CreateIndex("outbox_undispatched", "outbox", new[] { "dispatched_at", "position" }, "freediving", filter: "dispatched_at IS NULL");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "target_species", schema: "freediving");
            migrationBuilder.DropTable(name: "activities", schema: "freediving");
            migrationBuilder.DropTable(name: "outbox", schema: "freediving");
            migrationBuilder.DropTable(name: "processed_events", schema: "freediving");
        }
    }
}
