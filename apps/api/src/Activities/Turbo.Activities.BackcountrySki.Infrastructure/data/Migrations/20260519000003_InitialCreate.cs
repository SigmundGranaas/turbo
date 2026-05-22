using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;
using Turboapi.Activities.BackcountrySki.data;

#nullable disable

namespace Turboapi.Activities.BackcountrySki.data.Migrations
{
    /// <summary>
    /// Creates the backcountry-ski kind's typed storage: a single
    /// <c>backcountry_ski.activities</c> table with one typed column per
    /// kind-specific field, plus owned-collection tables
    /// <c>backcountry_ski.aspect_mix</c> and <c>backcountry_ski.legs</c>.
    /// Per-kind outbox + processed_events tables for transactional
    /// publishing + idempotent projection.
    /// </summary>
    [DbContext(typeof(BackcountrySkiContext))]
    [Migration("20260519000003_InitialCreate")]
    public partial class InitialCreate : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(name: "backcountry_ski");

            migrationBuilder.AlterDatabase()
                .Annotation("Npgsql:PostgresExtension:postgis", ",,");

            migrationBuilder.CreateTable(
                name: "activities",
                schema: "backcountry_ski",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    description = table.Column<string>(type: "text", nullable: true),
                    route = table.Column<LineString>(type: "geometry(LineString, 4326)", nullable: false),
                    ascent_meters = table.Column<int>(type: "integer", nullable: false),
                    descent_meters = table.Column<int>(type: "integer", nullable: false),
                    distance_meters = table.Column<int>(type: "integer", nullable: false),
                    elevation_min_meters = table.Column<int>(type: "integer", nullable: false),
                    elevation_max_meters = table.Column<int>(type: "integer", nullable: false),
                    ates_rating = table.Column<short>(type: "smallint", nullable: false),
                    dominant_aspect = table.Column<short>(type: "smallint", nullable: true),
                    varsom_region_id = table.Column<int>(type: "integer", nullable: true),
                    preferred_avalanche_max_level = table.Column<short>(type: "smallint", nullable: true),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    version = table.Column<long>(type: "bigint", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_backcountry_ski_activities", x => x.id);
                });

            migrationBuilder.CreateTable(
                name: "aspect_mix",
                schema: "backcountry_ski",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    aspect = table.Column<short>(type: "smallint", nullable: false),
                    fraction = table.Column<float>(type: "real", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_aspect_mix", x => new { x.activity_id, x.aspect });
                    table.ForeignKey(
                        name: "FK_aspect_mix_activities",
                        column: x => x.activity_id,
                        principalSchema: "backcountry_ski",
                        principalTable: "activities",
                        principalColumn: "id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "legs",
                schema: "backcountry_ski",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    ordinal = table.Column<int>(type: "integer", nullable: false),
                    leg_kind = table.Column<short>(type: "smallint", nullable: false),
                    start_elevation_meters = table.Column<int>(type: "integer", nullable: false),
                    end_elevation_meters = table.Column<int>(type: "integer", nullable: false),
                    geometry = table.Column<LineString>(type: "geometry(LineString, 4326)", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_legs", x => new { x.activity_id, x.ordinal });
                    table.ForeignKey(
                        name: "FK_legs_activities",
                        column: x => x.activity_id,
                        principalSchema: "backcountry_ski",
                        principalTable: "activities",
                        principalColumn: "id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "backcountry_ski",
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
                constraints: table =>
                {
                    table.PrimaryKey("PK_outbox", x => x.id);
                });

            migrationBuilder.CreateTable(
                name: "processed_events",
                schema: "backcountry_ski",
                columns: table => new
                {
                    event_id = table.Column<Guid>(type: "uuid", nullable: false),
                    processed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_processed_events", x => x.event_id);
                });

            migrationBuilder.CreateIndex(
                name: "idx_backcountry_ski_activities_route",
                schema: "backcountry_ski",
                table: "activities",
                column: "route")
                .Annotation("Npgsql:IndexMethod", "GIST");

            migrationBuilder.CreateIndex(
                name: "idx_backcountry_ski_activities_owner",
                schema: "backcountry_ski",
                table: "activities",
                column: "owner_id");

            migrationBuilder.CreateIndex(
                name: "idx_backcountry_ski_activities_owner_updated_at",
                schema: "backcountry_ski",
                table: "activities",
                columns: new[] { "owner_id", "updated_at" });

            migrationBuilder.CreateIndex(
                name: "outbox_aggregate",
                schema: "backcountry_ski",
                table: "outbox",
                columns: new[] { "aggregate_id", "position" });

            migrationBuilder.CreateIndex(
                name: "outbox_undispatched",
                schema: "backcountry_ski",
                table: "outbox",
                columns: new[] { "dispatched_at", "position" },
                filter: "dispatched_at IS NULL");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "legs", schema: "backcountry_ski");
            migrationBuilder.DropTable(name: "aspect_mix", schema: "backcountry_ski");
            migrationBuilder.DropTable(name: "activities", schema: "backcountry_ski");
            migrationBuilder.DropTable(name: "outbox", schema: "backcountry_ski");
            migrationBuilder.DropTable(name: "processed_events", schema: "backcountry_ski");
        }
    }
}
