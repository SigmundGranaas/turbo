
namespace Turboapi.Activity.domain.query;

public class ActivityQueryDto
{
    public Guid Position { get; set; }
    public Guid ActivityId { get; set; }
    public Guid OwnerId { get; set; }
    public string Name { get; set; }
    public string Description { get; set; }
    public string Icon { get; set; }

    public static ActivityQueryDto FromActivity(Activity activity)
    {
        return new ActivityQueryDto
        {
            Position = activity.Position,
            ActivityId = activity.Id,
            OwnerId = activity.OwnerId,
            Name = activity.Name,
            Description = activity.Description,
            Icon = activity.Icon,
        };
    }
}