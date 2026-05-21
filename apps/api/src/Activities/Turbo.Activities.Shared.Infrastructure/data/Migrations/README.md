# Activities — Migration generation notes

The migration `.cs` files in this folder carry the `[Migration]` and
`[DbContext]` attributes directly on the class (rather than as a partial
split with a separate `*.Designer.cs`). This is a valid EF Core pattern
and works at runtime via the standard `Migrate()` call wired in
`MigrateModuleDatabaseAsync<ActivitySummariesContext>` in each host.

There is no `*ModelSnapshot.cs` committed — generate it on first
`dotnet ef migrations add` after baseline by running:

```
dotnet ef migrations add NextChange --project src/Activities/Turbo.Activities.Shared.Infrastructure
```

EF will compute the snapshot from the current `OnModelCreating` plus the
already-applied migrations.
