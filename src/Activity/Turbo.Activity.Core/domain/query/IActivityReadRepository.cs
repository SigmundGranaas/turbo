namespace Turboapi.Activity.domain.query;

public interface IActivityReadRepository
{
    Task<Activity?> GetById(Guid id);
}