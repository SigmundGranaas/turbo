using Microsoft.EntityFrameworkCore.Migrations;
using NetTopologySuite.Geometries;
using Npgsql.EntityFrameworkCore.PostgreSQL.Metadata;

#nullable disable

namespace Turboapi.Activities.data.Migrations
{
    /// <inheritdoc />
    public partial class AddGeoRegions : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.CreateTable(
                name: "geo_regions",
                schema: "activities",
                columns: table => new
                {
                    id = table.Column<long>(type: "bigint", nullable: false)
                        .Annotation("Npgsql:ValueGenerationStrategy", NpgsqlValueGenerationStrategy.IdentityByDefaultColumn),
                    source = table.Column<string>(type: "text", nullable: false),
                    region_id = table.Column<string>(type: "text", nullable: false),
                    name = table.Column<string>(type: "text", nullable: false),
                    geometry = table.Column<Geometry>(type: "geometry(Geometry, 4326)", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_geo_regions", x => x.id);
                });

            migrationBuilder.CreateIndex(
                name: "idx_geo_regions_geometry",
                schema: "activities",
                table: "geo_regions",
                column: "geometry")
                .Annotation("Npgsql:IndexMethod", "GIST");

            migrationBuilder.CreateIndex(
                name: "idx_geo_regions_source_region_id",
                schema: "activities",
                table: "geo_regions",
                columns: new[] { "source", "region_id" },
                unique: true);
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(
                name: "geo_regions",
                schema: "activities");
        }
    }
}
