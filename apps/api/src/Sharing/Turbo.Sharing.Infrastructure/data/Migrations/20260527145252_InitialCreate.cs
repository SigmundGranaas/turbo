using System;
using Microsoft.EntityFrameworkCore.Migrations;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;

#nullable disable

namespace Turboapi.Sharing.data.Migrations
{
    /// <inheritdoc />
    public partial class InitialCreate : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(
                name: "sharing");

            migrationBuilder.CreateTable(
                name: "friendships",
                schema: "sharing",
                columns: table => new
                {
                    lower_user_id = table.Column<Guid>(type: "uuid", nullable: false),
                    higher_user_id = table.Column<Guid>(type: "uuid", nullable: false),
                    initiator_id = table.Column<Guid>(type: "uuid", nullable: false),
                    status = table.Column<string>(type: "text", nullable: false),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    accepted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_friendships", x => new { x.lower_user_id, x.higher_user_id });
                });

            migrationBuilder.CreateTable(
                name: "groups",
                schema: "sharing",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_groups", x => x.id);
                });

            migrationBuilder.CreateTable(
                name: "outbox",
                schema: "sharing",
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
                schema: "sharing",
                columns: table => new
                {
                    event_id = table.Column<Guid>(type: "uuid", nullable: false),
                    processed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_processed_events", x => x.event_id);
                });

            migrationBuilder.CreateTable(
                name: "resources",
                schema: "sharing",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    type = table.Column<string>(type: "text", nullable: false),
                    owner_id = table.Column<Guid>(type: "uuid", nullable: false),
                    visibility = table.Column<string>(type: "text", nullable: false),
                    version = table.Column<long>(type: "bigint", nullable: false),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    updated_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    deleted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_resources", x => x.id);
                });

            migrationBuilder.CreateTable(
                name: "share_invites",
                schema: "sharing",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    inviter_id = table.Column<Guid>(type: "uuid", nullable: false),
                    invitee_email = table.Column<string>(type: "text", nullable: false),
                    resource_id = table.Column<Guid>(type: "uuid", nullable: true),
                    role = table.Column<string>(type: "text", nullable: true),
                    created_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    expires_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    redeemed_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    redeemed_by_user_id = table.Column<Guid>(type: "uuid", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_share_invites", x => x.id);
                });

            migrationBuilder.CreateTable(
                name: "group_members",
                schema: "sharing",
                columns: table => new
                {
                    group_id = table.Column<Guid>(type: "uuid", nullable: false),
                    user_id = table.Column<Guid>(type: "uuid", nullable: false),
                    role = table.Column<string>(type: "text", nullable: false),
                    joined_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP")
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_group_members", x => new { x.group_id, x.user_id });
                    table.ForeignKey(
                        name: "FK_group_members_groups_group_id",
                        column: x => x.group_id,
                        principalSchema: "sharing",
                        principalTable: "groups",
                        principalColumn: "id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "grants",
                schema: "sharing",
                columns: table => new
                {
                    resource_id = table.Column<Guid>(type: "uuid", nullable: false),
                    subject_type = table.Column<string>(type: "text", nullable: false),
                    subject_id = table.Column<Guid>(type: "uuid", nullable: false),
                    role = table.Column<string>(type: "text", nullable: false),
                    granted_by = table.Column<Guid>(type: "uuid", nullable: false),
                    granted_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: false, defaultValueSql: "CURRENT_TIMESTAMP"),
                    expires_at = table.Column<DateTime>(type: "timestamp with time zone", nullable: true),
                    link_token = table.Column<string>(type: "text", nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_grants", x => new { x.resource_id, x.subject_type, x.subject_id });
                    table.ForeignKey(
                        name: "FK_grants_resources_resource_id",
                        column: x => x.resource_id,
                        principalSchema: "sharing",
                        principalTable: "resources",
                        principalColumn: "id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateIndex(
                name: "idx_friendships_higher_user",
                schema: "sharing",
                table: "friendships",
                column: "higher_user_id");

            migrationBuilder.CreateIndex(
                name: "idx_friendships_status_lower",
                schema: "sharing",
                table: "friendships",
                columns: new[] { "status", "lower_user_id" });

            migrationBuilder.CreateIndex(
                name: "idx_grants_link_token",
                schema: "sharing",
                table: "grants",
                column: "link_token",
                unique: true,
                filter: "link_token IS NOT NULL");

            migrationBuilder.CreateIndex(
                name: "idx_grants_subject",
                schema: "sharing",
                table: "grants",
                columns: new[] { "subject_type", "subject_id" });

            migrationBuilder.CreateIndex(
                name: "idx_group_members_user",
                schema: "sharing",
                table: "group_members",
                column: "user_id");

            migrationBuilder.CreateIndex(
                name: "idx_groups_owner",
                schema: "sharing",
                table: "groups",
                column: "owner_id");

            migrationBuilder.CreateIndex(
                name: "outbox_aggregate",
                schema: "sharing",
                table: "outbox",
                columns: new[] { "aggregate_id", "position" });

            migrationBuilder.CreateIndex(
                name: "outbox_undispatched",
                schema: "sharing",
                table: "outbox",
                columns: new[] { "dispatched_at", "position" },
                filter: "dispatched_at IS NULL");

            migrationBuilder.CreateIndex(
                name: "idx_resources_owner_type",
                schema: "sharing",
                table: "resources",
                columns: new[] { "owner_id", "type" });

            migrationBuilder.CreateIndex(
                name: "idx_resources_type_visibility",
                schema: "sharing",
                table: "resources",
                columns: new[] { "type", "visibility" });

            migrationBuilder.CreateIndex(
                name: "idx_resources_updated_at",
                schema: "sharing",
                table: "resources",
                column: "updated_at");

            migrationBuilder.CreateIndex(
                name: "idx_share_invites_email_pending",
                schema: "sharing",
                table: "share_invites",
                columns: new[] { "invitee_email", "redeemed_at" });
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(
                name: "friendships",
                schema: "sharing");

            migrationBuilder.DropTable(
                name: "grants",
                schema: "sharing");

            migrationBuilder.DropTable(
                name: "group_members",
                schema: "sharing");

            migrationBuilder.DropTable(
                name: "outbox",
                schema: "sharing");

            migrationBuilder.DropTable(
                name: "processed_events",
                schema: "sharing");

            migrationBuilder.DropTable(
                name: "share_invites",
                schema: "sharing");

            migrationBuilder.DropTable(
                name: "resources",
                schema: "sharing");

            migrationBuilder.DropTable(
                name: "groups",
                schema: "sharing");
        }
    }
}
