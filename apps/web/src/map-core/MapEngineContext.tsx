import { createContext, useContext, useState, type ReactNode } from 'react';
import type { MapEngine } from './engine';

/** Kills `mapRef` prop-drilling: the live engine is published once it boots and
 *  read anywhere below the provider via `useMapEngine()`. `map-engine`'s
 *  `<MapSurface>` is the sole publisher; features only ever read. */
interface EngineCtx {
  engine: MapEngine | null;
  /** Internal — only `map-engine`'s surface calls this (via `useMapEnginePublisher`). */
  setEngine: (e: MapEngine | null) => void;
}

const Ctx = createContext<EngineCtx | null>(null);

export function MapEngineProvider({ children }: { children: ReactNode }) {
  const [engine, setEngine] = useState<MapEngine | null>(null);
  return <Ctx.Provider value={{ engine, setEngine }}>{children}</Ctx.Provider>;
}

/** The live engine, or `null` until WebGPU/WASM has booted. */
export function useMapEngine(): MapEngine | null {
  return useContext(Ctx)?.engine ?? null;
}

/** Publisher seam for `map-engine`'s `<MapSurface>` — not for feature use. */
export function useMapEnginePublisher(): (e: MapEngine | null) => void {
  const c = useContext(Ctx);
  if (!c) throw new Error('useMapEnginePublisher must be used within <MapEngineProvider>');
  return c.setEngine;
}
