using Medo;
using Turboapi.Activity.domain.events;

namespace Turboapi.Activity.domain;

public class Activity: AggregateRoot
{
    public Guid Id { get; private set; }
    public Guid OwnerId { get; private set; }
    public Guid Position { get; private set; }
    public string Name { get; private set; }
    public string Description { get; private set; }
    public string Icon { get; private set; }
    
    public static Activity From(Guid id, Guid ownerId, Guid position, string name, string description, string icon)
    {
        var activity = new Activity
        {
            Id = id,  
            OwnerId = ownerId,
            Position = position,
            Name = name,
            Description = description,
            Icon = icon,
        };
        
        return activity;
    }
    
    public static Activity Create(Guid ownerId, Position position, string name, string description, string icon)
    {
        var positionId = Uuid7.NewGuid();
        
        var activity = new Activity
        {
            Id = Uuid7.NewUuid7(),  
            OwnerId = ownerId,
            Position = positionId,
            Name = name,
            Description = description,
            Icon = icon,
        };
        
        activity.AddEvent(new ActivityCreated(activity.Id, activity.OwnerId, activity.Position, activity.Name, activity.Description, activity.Icon));
        activity.AddEvent(new ActivityPositionCreated(positionId, position, activity.Id, activity.OwnerId));
        return activity;
    }

    public Activity Update(Guid user, string name, string description, string icon)
    {
        if (user != OwnerId)
        {
            throw new UnauthorizedAccessException("You are not authorized to edit this activity.");
        }
        
        Name = name;
        Description = description;
        Icon = icon;
        
        AddEvent(new ActivityUpdated(Id, Name, Description, Icon));
        return this;
    }
    
    public Activity Delete(Guid user)
    {
        if (user != OwnerId)
        {
            throw new UnauthorizedAccessException("You are not authorized to delete this activity.");
        }
        
        AddEvent(new ActivityDeleted(Id));
        return this;
    }

    public bool CanSeeActivity(Guid user)
    {
        return OwnerId == user;
    }
}