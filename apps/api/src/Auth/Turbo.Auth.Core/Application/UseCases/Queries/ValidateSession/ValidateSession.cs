namespace Turboapi.Auth.Application.UseCases.Queries.ValidateSession
{
    public record ValidateSessionResponse(
        Guid AccountId,
        string Email,
        IEnumerable<string> Roles,
        bool IsActive);
}