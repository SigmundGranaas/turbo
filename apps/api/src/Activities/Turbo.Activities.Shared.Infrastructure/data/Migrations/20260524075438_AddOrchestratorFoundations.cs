using System;
using System.Text.Json;
using Microsoft.EntityFrameworkCore.Migrations;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;

#nullable disable

namespace Turboapi.Activities.data.Migrations
{
    /// <summary>
    /// Foundations for the orchestrator architecture: persistent conditions
    /// snapshots (history beyond the TTL cache), user-contributed
    /// observations + visits, and geometry-derived geo-context. Each table
    /// is additive — existing tables (<c>activity_summaries</c>,
    /// <c>conditions_cache</c>, outbox/processed_events) stay untouched.
    /// </summary>
    public partial class AddOrchestratorFoundations : Migration
    {
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.CreateTable(
                name: "conditions_snapshots",
                schema: "activities",
                columns: table => new
                {
                    id = table.Column<long>(type: "bigint", nullable: false)
                        .Annotation("Npgsql:ValueGenerationStrategy", NpgsqlValueGenerationStrategy.IdentityByDefaultColumn),
                    provider_key = table.Column<string>(type: "text", nullable: false),
                    grid_cell = table.Column<string>(type: "text", nullable: false),
                    observed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    fetched_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    payload = table.Column<string>(type: "jsonb", nullable: false),
                    payload_schema_version = table.Column<short>(type: "smallint", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_conditions_snapshots", x => x.id);
                });

            migrationBuilder.CreateIndex(
                name: "idx_conditions_snapshots_provider_grid_observed_at",
                schema: "activities",
                table: "conditions_snapshots",
                columns: new[] { "provider_key", "grid_cell", "observed_at" },
                descending: new[] { false, false, true });

            migrationBuilder.CreateTable(
                name: "activity_observations",
                schema: "activities",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    user_id = table.Column<Guid>(type: "uuid", nullable: false),
                    observed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    kind = table.Column<string>(type: "text", nullable: false),
                    rating = table.Column<short>(type: "smallint", nullable: true),
                    comment = table.Column<string>(type: "text", nullable: true),
                    kind_payload = table.Column<JsonDocument>(type: "jsonb", nullable: false),
                    photo_count = table.Column<short>(type: "smallint", nullable: false, defaultValue: (short)0),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    watershed_href_id = table.Column<string>(type: "text", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_activity_observations", x => x.id);
                });

            migrationBuilder.CreateIndex(
                name: "idx_activity_observations_activity_observed_at",
                schema: "activities",
                table: "activity_observations",
                columns: new[] { "activity_id", "observed_at" },
                descending: new[] { false, true });

            migrationBuilder.CreateIndex(
                name: "idx_activity_observations_user_kind_observed_at",
                schema: "activities",
                table: "activity_observations",
                columns: new[] { "user_id", "kind", "observed_at" },
                descending: new[] { false, false, true });

            migrationBuilder.CreateIndex(
                name: "idx_activity_observations_watershed_observed_at",
                schema: "activities",
                table: "activity_observations",
                columns: new[] { "watershed_href_id", "observed_at" },
                descending: new[] { false, true });

            migrationBuilder.CreateTable(
                name: "activity_visits",
                schema: "activities",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    user_id = table.Column<Guid>(type: "uuid", nullable: false),
                    visited_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    source = table.Column<string>(type: "text", nullable: false),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_activity_visits", x => x.id);
                });

            migrationBuilder.CreateIndex(
                name: "idx_activity_visits_activity_visited_at",
                schema: "activities",
                table: "activity_visits",
                columns: new[] { "activity_id", "visited_at" },
                descending: new[] { false, true });

            migrationBuilder.CreateIndex(
                name: "idx_activity_visits_user_visited_at",
                schema: "activities",
                table: "activity_visits",
                columns: new[] { "user_id", "visited_at" },
                descending: new[] { false, true });

            migrationBuilder.CreateTable(
                name: "activity_geo_contexts",
                schema: "activities",
                columns: table => new
                {
                    activity_id = table.Column<Guid>(type: "uuid", nullable: false),
                    version = table.Column<int>(type: "integer", nullable: false),
                    geom_hash = table.Column<string>(type: "text", nullable: false),
                    payload = table.Column<JsonDocument>(type: "jsonb", nullable: false),
                    computed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_activity_geo_contexts", x => x.activity_id);
                });
        }

        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(name: "activity_geo_contexts", schema: "activities");
            migrationBuilder.DropTable(name: "activity_visits", schema: "activities");
            migrationBuilder.DropTable(name: "activity_observations", schema: "activities");
            migrationBuilder.DropTable(name: "conditions_snapshots", schema: "activities");
        }
    }
}
