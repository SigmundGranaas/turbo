using System;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;
using Turboapi.Activities.XcSki.data;

#nullable disable

namespace Turboapi.Activities.XcSki.data.Migrations
{
    [DbContext(typeof(XcSkiContext))]
    [Migration("20260519000005_InitialCreate")]
    public partial class InitialCreate : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(name: "xc_ski");
            migrationBuilder.AlterDatabase().Annotation("Npgsql:PostgresExtension:postgis", ",,");

            migrationBuilder.CreateTable(
                name: "activities",
                schema: "xc_ski",
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
                    technique = table.Column<short>(type: "smallint", nullable: false),
                    grooming_status = table.Column<short>(type: "smallint", nullable: false),
                    is_lit = table.Column<bool>(type: "boolean", nullable: false),
                    requires_season_pass = table.Column<bool>(type: "boolean", nullable: false),
                    grooming_feed_key = table.Column<string>(type: "text", nullable: true),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    version = table.Column<long>(type: "bigint", nullable: false)
                },
                constraints: table => { table.PrimaryKey("PK_xc_ski_activities", x => x.id); });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "xc_ski",
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
                schema: "xc_ski",
                columns: table => new
                {
                    event_id = table.Column<Guid>(type: "uuid", nullable: false),
                    processed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table => { table.PrimaryKey("PK_processed_events", x => x.event_id); });

            migrationBuilder.CreateIndex("idx_xc_ski_activities_route", "activities", "route", "xc_ski").Annotation("Npgsql:IndexMethod", "GIST");
            migrationBuilder.CreateIndex("idx_xc_ski_activities_owner", "activities", "owner_id", "xc_ski");
            migrationBuilder.CreateIndex("idx_xc_ski_activities_owner_updated_at", "activities", new[] { "owner_id", "updated_at" }, "xc_ski");
            migrationBuilder.CreateIndex("outbox_aggregate", "outbox", new[] { "aggregate_id", "position" }, "xc_ski");
            migrationBuilder.CreateIndex("outbox_undispatched", "outbox", new[] { "dispatched_at", "position" }, "xc_ski", filter: "dispatched_at IS NULL");
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "activities", schema: "xc_ski");
            migrationBuilder.DropTable(name: "outbox", schema: "xc_ski");
            migrationBuilder.DropTable(name: "processed_events", schema: "xc_ski");
        }
    }
}
