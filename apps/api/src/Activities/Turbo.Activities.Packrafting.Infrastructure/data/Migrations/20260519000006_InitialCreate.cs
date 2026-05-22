using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;
using Turboapi.Activities.Packrafting.data;

#nullable disable

namespace Turboapi.Activities.Packrafting.data.Migrations
{
    [DbContext(typeof(PackraftingContext))]
    [Migration("20260519000006_InitialCreate")]
    public partial class InitialCreate : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(name: "packrafting");
            migrationBuilder.AlterDatabase().Annotation("Npgsql:PostgresExtension:postgis", ",,");

            migrationBuilder.CreateTable(
                name: "activities",
                schema: "packrafting",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    description = table.Column<string>(type: "text", nullable: true),
                    route = table.Column<LineString>(type: "geometry(LineString, 4326)", nullable: false),
                    distance_meters = table.Column<int>(type: "integer", nullable: false),
                    paddle_distance_meters = table.Column<int>(type: "integer", nullable: false),
                    portage_distance_meters = table.Column<int>(type: "integer", nullable: false),
                    max_grade = table.Column<short>(type: "smallint", nullable: false),
                    typical_grade = table.Column<short>(type: "smallint", nullable: false),
                    put_in_lat = table.Column<double>(type: "double precision", nullable: false),
                    put_in_lon = table.Column<double>(type: "double precision", nullable: false),
                    take_out_lat = table.Column<double>(type: "double precision", nullable: false),
                    take_out_lon = table.Column<double>(type: "double precision", nullable: false),
                    nve_station_code = table.Column<string>(type: "text", nullable: true),
                    min_flow_cumecs = table.Column<float>(type: "real", nullable: true),
                    max_flow_cumecs = table.Column<float>(type: "real", nullable: true),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    version = table.Column<long>(type: "bigint", nullable: false)
                },
                constraints: table => { table.PrimaryKey("PK_packrafting_activities", x => x.id); });

            migrationBuilder.CreateTable(
                name: "segments",
                schema: "packrafting",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    ordinal = table.Column<int>(type: "integer", nullable: false),
                    kind = table.Column<short>(type: "smallint", nullable: false),
                    grade = table.Column<short>(type: "smallint", nullable: true),
                    distance_meters = table.Column<int>(type: "integer", nullable: false),
                    geometry = table.Column<LineString>(type: "geometry(LineString, 4326)", nullable: false),
                    notes = table.Column<string>(type: "text", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_packrafting_segments", x => new { x.activity_id, x.ordinal });
                    table.ForeignKey("FK_segments_activities", x => x.activity_id, "activities", "id", principalSchema: "packrafting", onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "packrafting",
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
                schema: "packrafting",
                columns: table => new
                {
                    event_id = table.Column<Guid>(type: "uuid", nullable: false),
                    processed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table => { table.PrimaryKey("PK_processed_events", x => x.event_id); });

            migrationBuilder.CreateIndex("idx_packrafting_activities_route", "activities", "route", "packrafting").Annotation("Npgsql:IndexMethod", "GIST");
            migrationBuilder.CreateIndex("idx_packrafting_activities_owner", "activities", "owner_id", "packrafting");
            migrationBuilder.CreateIndex("idx_packrafting_activities_owner_updated_at", "activities", new[] { "owner_id", "updated_at" }, "packrafting");
            migrationBuilder.CreateIndex("outbox_aggregate", "outbox", new[] { "aggregate_id", "position" }, "packrafting");
            migrationBuilder.CreateIndex("outbox_undispatched", "outbox", new[] { "dispatched_at", "position" }, "packrafting", filter: "dispatched_at IS NULL");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "segments", schema: "packrafting");
            migrationBuilder.DropTable(name: "activities", schema: "packrafting");
            migrationBuilder.DropTable(name: "outbox", schema: "packrafting");
            migrationBuilder.DropTable(name: "processed_events", schema: "packrafting");
        }
    }
}
