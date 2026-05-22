using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;
using Turboapi.Activities.data;

#nullable disable

namespace Turboapi.Activities.data.Migrations
{
    /// <summary>
    /// Bootstrap migration for the cross-kind summaries projection. Creates
    /// the <c>activities</c> schema, the <c>activity_summaries</c> table
    /// (mixed geometry type — Point, LineString, Polygon all live here),
    /// outbox + processed_events tables, and the spatial/owner/kind
    /// indexes the read endpoints rely on.
    /// </summary>
    [DbContext(typeof(ActivitySummariesContext))]
    [Migration("20260519000001_InitialCreate")]
    public partial class InitialCreate : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(name: "activities");

            migrationBuilder.AlterDatabase()
                .Annotation("Npgsql:PostgresExtension:postgis", ",,");

            migrationBuilder.CreateTable(
                name: "activity_summaries",
                schema: "activities",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    kind = table.Column<string>(type: "text", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    geometry = table.Column<Geometry>(type: "geometry(Geometry, 4326)", nullable: false),
                    icon_key = table.Column<string>(type: "text", nullable: false),
                    color_hex = table.Column<string>(type: "text", nullable: true),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    version = table.Column<long>(type: "bigint", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_activity_summaries", x => x.id);
                });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "activities",
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
                schema: "activities",
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
                name: "idx_activity_summaries_geometry",
                schema: "activities",
                table: "activity_summaries",
                column: "geometry")
                .Annotation("Npgsql:IndexMethod", "GIST");

            migrationBuilder.CreateIndex(
                name: "idx_activity_summaries_owner",
                schema: "activities",
                table: "activity_summaries",
                column: "owner_id");

            migrationBuilder.CreateIndex(
                name: "idx_activity_summaries_owner_updated_at",
                schema: "activities",
                table: "activity_summaries",
                columns: new[] { "owner_id", "updated_at" });

            migrationBuilder.CreateIndex(
                name: "idx_activity_summaries_kind",
                schema: "activities",
                table: "activity_summaries",
                column: "kind");

            migrationBuilder.CreateIndex(
                name: "outbox_aggregate",
                schema: "activities",
                table: "outbox",
                columns: new[] { "aggregate_id", "position" });

            migrationBuilder.CreateIndex(
                name: "outbox_undispatched",
                schema: "activities",
                table: "outbox",
                columns: new[] { "dispatched_at", "position" },
                filter: "dispatched_at IS NULL");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "activity_summaries", schema: "activities");
            migrationBuilder.DropTable(name: "outbox", schema: "activities");
            migrationBuilder.DropTable(name: "processed_events", schema: "activities");
        }
    }
}
