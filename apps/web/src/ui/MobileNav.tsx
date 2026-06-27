import { Glass } from './Glass';
import { Icon } from './Icon';
import { Cookie } from './Cookie';

const ITEMS = [
  { id: 'explore', icon: 'explore', label: 'Map' },
  { id: 'saved', icon: 'bookmark', label: 'Saved' },
  { id: 'conditions', icon: 'partly_cloudy_day', label: 'Weather' },
  { id: 'activities', icon: 'directions_walk', label: 'Activities' },
] as const;

/** Bottom navigation bar for mobile — the destinations the desktop left
 *  NavRail holds (Explore/Saved/Conditions/Activities + Account), which is
 *  hidden on narrow screens. Without this, Conditions/Activities/Account have
 *  no entry point on a phone. Shown only when no panel/sheet is up. */
export function MobileNav({
  dark,
  active,
  signedIn = false,
  avatar = 'S',
  onNav,
  onAccount,
}: {
  dark: boolean;
  active: string;
  signedIn?: boolean;
  avatar?: string;
  onNav: (id: string) => void;
  onAccount: () => void;
}) {
  const cell = (on: boolean): React.CSSProperties => ({
    flex: 1,
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: 2,
    padding: '7px 0 5px',
    border: 'none',
    background: 'transparent',
    cursor: 'pointer',
    color: on ? 'var(--primary)' : 'var(--on-surface-variant)',
    font: '600 10px/12px var(--font-sans)',
  });
  return (
    <Glass dark={dark} radius={22} style={{ padding: '2px 6px', display: 'flex', alignItems: 'stretch' }}>
      {ITEMS.map((it) => {
        const on = active === it.id;
        return (
          <button className="tm-icon-btn" key={it.id} title={it.label} onClick={() => onNav(it.id)} style={cell(on)}>
            <Icon name={it.icon} size={24} fill={on} />
            {it.label}
          </button>
        );
      })}
      <button className="tm-icon-btn" title="Account" onClick={onAccount} style={cell(active === 'account')}>
        {signedIn ? (
          <Cookie size={24} fill="var(--primary)">
            <span style={{ font: '800 11px/1 var(--font-sans)', color: 'var(--on-primary)' }}>{avatar}</span>
          </Cookie>
        ) : (
          <Icon name="account_circle" size={24} fill={active === 'account'} />
        )}
        Account
      </button>
    </Glass>
  );
}
