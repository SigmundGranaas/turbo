namespace Turboapi.Activity.domain.command;

public class EditActivityCommand
{
    public Guid ActivityID { get; set; }
    public Guid UserID { get; set; }
    public string Name { get; set; }
    public string Description { get; set; }
    public string Icon { get; set; }
}

public class UpdateNameCommand
{
    public Guid ActivityID { get; set; }
    public Guid UserID { get; set; }
    public string Name { get; set; }
}

public class UpdateDescriptionCommand
{
    public Guid ActivityID { get; set; }
    public Guid UserID { get; set; }
    public string Description { get; set; }
}

public class EditIconCommand
{
    public Guid ActivityID { get; set; }
    public Guid UserID { get; set; }
    public string Icon { get; set; }
}