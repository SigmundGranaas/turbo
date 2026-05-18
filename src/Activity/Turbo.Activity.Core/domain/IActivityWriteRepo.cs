using Turboapi.Activity.domain.query;

namespace Turboapi.Activity.data;

public interface IActivityWriteRepository
{
    Task<ActivityQueryDto?> GetById(Guid id);
    Task Add(ActivityQueryDto entity);
    Task Update(ActivityQueryDto entity);
    Task Delete(ActivityQueryDto entity);
}