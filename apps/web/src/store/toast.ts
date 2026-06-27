import { create } from 'zustand';

/** App-wide transient toast. Any slice can surface a message
 *  (`useToast.getState().show('…')`); the host renders the single toast pill.
 *  Lives in the shared store tier so feature mutations can report failures
 *  without reaching into the host. */
interface ToastState {
  message: string | null;
  show: (message: string) => void;
  clear: () => void;
}

let timer: ReturnType<typeof setTimeout> | undefined;

export const useToast = create<ToastState>((set) => ({
  message: null,
  show: (message) => {
    if (timer) clearTimeout(timer);
    set({ message });
    timer = setTimeout(() => set({ message: null }), 3200);
  },
  clear: () => {
    if (timer) clearTimeout(timer);
    set({ message: null });
  },
}));
