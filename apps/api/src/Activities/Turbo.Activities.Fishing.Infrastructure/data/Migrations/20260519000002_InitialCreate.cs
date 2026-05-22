using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;
using Turboapi.Activities.Fishing.data;

#nullable disable

namespace Turboapi.Activities.Fishing.data.Migrations
{
    /// <summary>
    /// Creates the fishing kind's typed storage: a single
    /// <c>fishing.activities</c> table with one typed column per
    /// fishing-specific field, plus owned-collection tables for target
    /// species and depth samples. No JSONB anywhere. Outbox +
    /// processed_events tables in the same schema for transactional
    /// publish + idempotent projection.
    /// </summary>
    [DbContext(typeof(FishingContext))]
    [Migration("20260519000002_InitialCreate")]
    public partial class InitialCreate : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(name: "fishing");

            migrationBuilder.AlterDatabase()
                .Annotation("Npgsql:PostgresExtension:postgis", ",,");

            migrationBuilder.CreateTable(
                name: "activities",
                schema: "fishing",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    description = table.Column<string>(type: "text", nullable: true),
                    geometry = table.Column<Point>(type: "geometry(Point, 4326)", nullable: false),
                    water_kind = table.Column<short>(type: "smallint", nullable: false),
                    shore_or_boat = table.Column<short>(type: "smallint", nullable: false),
                    access_notes = table.Column<string>(type: "text", nullable: true),
                    preferred_pressure_min_hpa = table.Column<short>(type: "smallint", nullable: true),
                    preferred_pressure_max_hpa = table.Column<short>(type: "smallint", nullable: true),
                    preferred_wind_max_ms = table.Column<float>(type: "real", nullable: true),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    version = table.Column<long>(type: "bigint", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_fishing_activities", x => x.id);
                });

            migrationBuilder.CreateTable(
                name: "target_species",
                schema: "fishing",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    species_code = table.Column<string>(type: "text", nullable: false),
                    notes = table.Column<string>(type: "text", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_target_species", x => new { x.activity_id, x.species_code });
                    table.ForeignKey(
                        name: "FK_target_species_activities",
                        column: x => x.activity_id,
                        principalSchema: "fishing",
                        principalTable: "activities",
                        principalColumn: "id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "depth_samples",
                schema: "fishing",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    ordinal = table.Column<int>(type: "integer", nullable: false),
                    lat = table.Column<double>(type: "double precision", nullable: false),
                    lon = table.Column<double>(type: "double precision", nullable: false),
                    depth_meters = table.Column<float>(type: "real", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_depth_samples", x => new { x.activity_id, x.ordinal });
                    table.ForeignKey(
                        name: "FK_depth_samples_activities",
                        column: x => x.activity_id,
                        principalSchema: "fishing",
                        principalTable: "activities",
                        principalColumn: "id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "fishing",
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
                schema: "fishing",
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
                name: "idx_fishing_activities_geometry",
                schema: "fishing",
                table: "activities",
                column: "geometry")
                .Annotation("Npgsql:IndexMethod", "GIST");

            migrationBuilder.CreateIndex(
                name: "idx_fishing_activities_owner",
                schema: "fishing",
                table: "activities",
                column: "owner_id");

            migrationBuilder.CreateIndex(
                name: "idx_fishing_activities_owner_updated_at",
                schema: "fishing",
                table: "activities",
                columns: new[] { "owner_id", "updated_at" });

            migrationBuilder.CreateIndex(
                name: "outbox_aggregate",
                schema: "fishing",
                table: "outbox",
                columns: new[] { "aggregate_id", "position" });

            migrationBuilder.CreateIndex(
                name: "outbox_undispatched",
                schema: "fishing",
                table: "outbox",
                columns: new[] { "dispatched_at", "position" },
                filter: "dispatched_at IS NULL");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "depth_samples", schema: "fishing");
            migrationBuilder.DropTable(name: "target_species", schema: "fishing");
            migrationBuilder.DropTable(name: "activities", schema: "fishing");
            migrationBuilder.DropTable(name: "outbox", schema: "fishing");
            migrationBuilder.DropTable(name: "processed_events", schema: "fishing");
        }
    }
}
