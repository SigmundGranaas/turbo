import { describe, it, expect, beforeEach } from 'vitest';
import { usePanelHost } from './panelHost';

/** Behaviour of the side-panel slot the user sees: exactly one panel shows (or
 *  none), and opening one replaces whatever was open — there is no way to have
 *  two panels visible at once. */
describe('panel host (single visible slot)', () => {
  beforeEach(() => usePanelHost.getState().close());

  it('starts with nothing shown', () => {
    expect(usePanelHost.getState().active).toBeNull();
  });

  it('shows the panel that was opened', () => {
    usePanelHost.getState().open('marker-detail');
    expect(usePanelHost.getState().active).toBe('marker-detail');
  });

  it('opening a second panel hides the first (never two at once)', () => {
    usePanelHost.getState().open('saved');
    usePanelHost.getState().open('account');
    expect(usePanelHost.getState().active).toBe('account');
  });

  it('closing returns to nothing shown', () => {
    usePanelHost.getState().open('conditions');
    usePanelHost.getState().close();
    expect(usePanelHost.getState().active).toBeNull();
  });
});
