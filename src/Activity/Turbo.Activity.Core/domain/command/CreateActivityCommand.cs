namespace Turboapi.Activity.domain.command;

public class CreateActivityCommand
{ 
   public Guid OwnerId { get; set; }
   public Position Position { get; set; }
   public string Name { get; set; }
   public string Description { get; set; }
   public string Icon { get; set; }
}