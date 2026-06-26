import { useQuery } from '@tanstack/react-query';
import { getConditions } from './conditions';

/** Weather at a point. Keyed by ~1km-rounded coords so nearby selections share
 *  a cache entry and don't refetch on sub-km jitter. */
export function useConditions(lat?: number, lng?: number) {
  const rlat = lat != null ? Math.round(lat * 100) / 100 : undefined;
  const rlng = lng != null ? Math.round(lng * 100) / 100 : undefined;
  return useQuery({
    queryKey: ['conditions', rlat, rlng],
    queryFn: () => getConditions(rlat!, rlng!),
    enabled: rlat != null && rlng != null,
    staleTime: 10 * 60_000,
  });
}
