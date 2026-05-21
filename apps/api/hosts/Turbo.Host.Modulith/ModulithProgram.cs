namespace Turbo.Host.Modulith;

/// <summary>
/// Marker for <see cref="Microsoft.AspNetCore.Mvc.Testing.WebApplicationFactory{TEntryPoint}"/>:
/// resolving against this type points the test host at the modulith
/// assembly's top-level <c>Program</c>, which would otherwise collide
/// with the per-host <c>Program</c> classes the modulith transitively
/// references via Turbo.{Auth,Activity,Geo}.Api.
/// </summary>
public class ModulithProgram;
