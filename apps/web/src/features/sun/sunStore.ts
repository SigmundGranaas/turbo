import { create } from 'zustand';

const nowHour = (): number => {
  const d = new Date();
  return d.getHours() + d.getMinutes() / 60;
};

/** The sun tool's own state: on/off + the time-of-day (hours past local midnight)
 *  the slider sweeps. Owned by the slice (was scattered across `uiStore.sun` +
 *  a local `sunHour` in MapScreen). */
interface SunState {
  on: boolean;
  hour: number;
  setOn: (v: boolean) => void;
  setHour: (h: number) => void;
}

export const useSunStore = create<SunState>((set) => ({
  on: false,
  hour: nowHour(),
  setOn: (on) => set({ on }),
  setHour: (hour) => set({ hour }),
}));

export { nowHour };
