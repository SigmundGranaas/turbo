using Microsoft.EntityFrameworkCore;
using Turboapi.Activity.domain.query;

namespace Turboapi.Activity.data;

public class ActivityWriteRepository : IActivityWriteRepository
{
    private readonly ActivityContext _context;

    public ActivityWriteRepository(ActivityContext context)
    {
        _context = context;
    }

    public async Task<ActivityQueryDto?> GetById(Guid id)
    {
        return await _context.Activities
            .FirstOrDefaultAsync(a => a.ActivityId == id);
    }
    
    public async Task Add(ActivityQueryDto dto)
    {
       _context.Activities.Add(dto);
        await _context.SaveChangesAsync();
    }
    public async Task Update(ActivityQueryDto dto)
    {
        var res = _context.Activities.Update(dto);
        await _context.SaveChangesAsync();
     
    }
    public async Task Delete(ActivityQueryDto dto)
    {
        _context.Activities.Remove(dto);
        await _context.SaveChangesAsync();
    }
}