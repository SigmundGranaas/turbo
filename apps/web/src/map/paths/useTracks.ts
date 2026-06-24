import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createTrack, deleteTrack, listTracks, type Track, type TrackInput } from '../../api/tracks';

const KEY = ['tracks'];

export function useTracks() {
  return useQuery({ queryKey: KEY, queryFn: listTracks, staleTime: 30_000 });
}

export function useCreateTrack() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: TrackInput) => createTrack(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useDeleteTrack() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (t: Track) => deleteTrack(t),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}
