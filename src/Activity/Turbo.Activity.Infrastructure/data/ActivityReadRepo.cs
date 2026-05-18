using Microsoft.EntityFrameworkCore;
using Turboapi.Activity.domain.query;

namespace Turboapi.Activity.data;

public class ActivityReadRepository : IActivityReadRepository
{
    private readonly ActivityContext _context;

    public ActivityReadRepository(ActivityContext context)
    {
        _context = context;
    }

    public async Task<domain.Activity?> GetById(Guid id)
    {
        var dto = await _context.Activities
            .FirstOrDefaultAsync(a => a.ActivityId == id);

        return dto == null ? null : domain.Activity.From(dto.ActivityId, dto.OwnerId, dto.Position, dto.Name, dto.Description, dto.Icon);
    }
}