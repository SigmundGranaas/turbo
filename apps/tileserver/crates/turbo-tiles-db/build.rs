// `sqlx::migrate!` embeds the migration files at COMPILE time, but cargo has
// no idea the macro reads `../../migrations` — so adding a migration file
// does not, by itself, recompile this crate, and a stale binary silently
// skips the new migration at boot. (This bit twice during the basemap work:
// `terrain.contour` and `terrain.coastline` both "applied" as no-ops until a
// manual `touch src/migrations.rs`.) Declaring the directory here makes
// cargo rescan it on every build and rebuild when anything inside changes.
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
