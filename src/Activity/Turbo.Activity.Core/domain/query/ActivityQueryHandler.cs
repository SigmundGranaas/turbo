namespace Turboapi.Activity.domain.query;

public class ActivityQueryHandler
{
    private readonly IActivityReadRepository _repository;

    public ActivityQueryHandler(IActivityReadRepository repository)
    {
        _repository = repository;
    }

    public async Task<ActivityQueryDto?> Handle(ActivityQuery.GetActivityByIdQuery query)
    {
        var activity = await _repository.GetById(query.ActivityId);
        if (activity == null)
        {
            return null;
        }

        if (!activity.CanSeeActivity(query.UserId))
        {
            // Privacy-preserving: another user's activity is indistinguishable
            // from a missing one. The controller maps null to 404.
            return null;
        }

        return new ActivityQueryDto()
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