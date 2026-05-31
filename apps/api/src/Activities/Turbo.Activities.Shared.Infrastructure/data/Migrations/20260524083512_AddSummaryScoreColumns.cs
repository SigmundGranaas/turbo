using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Turboapi.Activities.data.Migrations
{
    /// <inheritdoc />
    public partial class AddSummaryScoreColumns : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.AddColumn<int>(
                name: "summary_score",
                schema: "activities",
                table: "activity_summaries",
                type: "integer",
                nullable: true);

            migrationBuilder.AddColumn<DateTime>(
                name: "summary_score_at",
                schema: "activities",
                table: "activity_summaries",
                type: "timestamp with time zone",
                nullable: true);

            migrationBuilder.AddColumn<string>(
                name: "top_driver_label",
                schema: "activities",
                table: "activity_summaries",
                type: "text",
                nullable: true);
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropColumn(
                name: "summary_score",
                schema: "activities",
                table: "activity_summaries");

            migrationBuilder.DropColumn(
                name: "summary_score_at",
                schema: "activities",
                table: "activity_summaries");

            migrationBuilder.DropColumn(
                name: "top_driver_label",
                schema: "activities",
                table: "activity_summaries");
        }
    }
}
