import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createMarker,
  deleteMarker,
  listMarkers,
  updateMarker,
  type Marker,
  type MarkerInput,
} from './api';

const KEY = ['markers'];

export function useMarkers() {
  return useQuery({ queryKey: KEY, queryFn: listMarkers, staleTime: 30_000 });
}

export function useCreateMarker() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: MarkerInput) => createMarker(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useUpdateMarker() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (m: Marker) => updateMarker(m),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useDeleteMarker() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (m: Marker) => deleteMarker(m),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}
