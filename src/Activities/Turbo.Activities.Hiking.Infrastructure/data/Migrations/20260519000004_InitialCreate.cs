using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;
using Turboapi.Activities.Hiking.data;

#nullable disable

namespace Turboapi.Activities.Hiking.data.Migrations
{
    [DbContext(typeof(HikingContext))]
    [Migration("20260519000004_InitialCreate")]
    public partial class InitialCreate : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(name: "hiking");
            migrationBuilder.AlterDatabase().Annotation("Npgsql:PostgresExtension:postgis", ",,");

            migrationBuilder.CreateTable(
                name: "activities",
                schema: "hiking",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    description = table.Column<string>(type: "text", nullable: true),
                    route = table.Column<LineString>(type: "geometry(LineString, 4326)", nullable: false),
                    distance_meters = table.Column<int>(type: "integer", nullable: false),
                    ascent_meters = table.Column<int>(type: "integer", nullable: false),
                    descent_meters = table.Column<int>(type: "integer", nullable: false),
                    elevation_min_meters = table.Column<int>(type: "integer", nullable: false),
                    elevation_max_meters = table.Column<int>(type: "integer", nullable: false),
                    difficulty = table.Column<short>(type: "smallint", nullable: false),
                    surface = table.Column<short>(type: "smallint", nullable: false),
                    marking = table.Column<short>(type: "smallint", nullable: false),
                    estimated_hours = table.Column<float>(type: "real", nullable: true),
                    has_water_sources = table.Column<bool>(type: "boolean", nullable: false),
                    has_shelter = table.Column<bool>(type: "boolean", nullable: false),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    version = table.Column<long>(type: "bigint", nullable: false)
                },
                constraints: table => { table.PrimaryKey("PK_hiking_activities", x => x.id); });

            migrationBuilder.CreateTable(
                name: "water_sources",
                schema: "hiking",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    ordinal = table.Column<int>(type: "integer", nullable: false),
                    lat = table.Column<double>(type: "double precision", nullable: false),
                    lon = table.Column<double>(type: "double precision", nullable: false),
                    kind = table.Column<string>(type: "text", nullable: false),
                    notes = table.Column<string>(type: "text", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_water_sources", x => new { x.activity_id, x.ordinal });
                    table.ForeignKey("FK_water_sources_activities", x => x.activity_id, "activities", "id", principalSchema: "hiking", onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "hiking",
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
                schema: "hiking",
                columns: table => new
                {
                    event_id = table.Column<Guid>(type: "uuid", nullable: false),
                    processed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table => { table.PrimaryKey("PK_processed_events", x => x.event_id); });

            migrationBuilder.CreateIndex("idx_hiking_activities_route", "activities", "route", "hiking").Annotation("Npgsql:IndexMethod", "GIST");
            migrationBuilder.CreateIndex("idx_hiking_activities_owner", "activities", "owner_id", "hiking");
            migrationBuilder.CreateIndex("idx_hiking_activities_owner_updated_at", "activities", new[] { "owner_id", "updated_at" }, "hiking");
            migrationBuilder.CreateIndex("outbox_aggregate", "outbox", new[] { "aggregate_id", "position" }, "hiking");
            migrationBuilder.CreateIndex("outbox_undispatched", "outbox", new[] { "dispatched_at", "position" }, "hiking", filter: "dispatched_at IS NULL");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "water_sources", schema: "hiking");
            migrationBuilder.DropTable(name: "activities", schema: "hiking");
            migrationBuilder.DropTable(name: "outbox", schema: "hiking");
            migrationBuilder.DropTable(name: "processed_events", schema: "hiking");
        }
    }
}
