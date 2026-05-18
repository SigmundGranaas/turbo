namespace Turboapi.Activity.domain.command;

public class DeleteActivityCommand
{
    public Guid ActivityID { get; set; }
    public Guid UserID { get; set; }
}
