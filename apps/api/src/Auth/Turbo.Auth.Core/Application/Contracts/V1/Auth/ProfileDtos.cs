namespace Turboapi.Auth.Application.Contracts.V1.Auth
{
    public record ChangePasswordRequest(
        string CurrentPassword,
        string NewPassword,
        string ConfirmNewPassword);

    public record UpdateProfileRequest(
        string? DisplayName);

    public record ProfileResponse(
        Guid AccountId,
        string Email,
        string? DisplayName);
}
