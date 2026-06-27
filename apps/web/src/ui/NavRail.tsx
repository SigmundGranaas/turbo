import { Glass } from './Glass';
import { Icon } from './Icon';
import { Cookie } from './Cookie';

export interface NavItem {
  id: string;
  icon: string;
  label: string;
}

export const NAV_ITEMS: NavItem[] = [
  { id: 'explore', icon: 'explore', label: 'Explore' },
  { id: 'saved', icon: 'bookmark', label: 'Saved' },
  { id: 'conditions', icon: 'partly_cloudy_day', label: 'Conditions' },
  { id: 'activities', icon: 'directions_walk', label: 'Activities' },
];

/** Desktop app-shell nav rail (left): brand Cookie, primary destinations, and
 *  the account avatar / sign-in at the foot. Ported from the design's `NavRail`. */
export function NavRail({
  dark,
  active = 'explore',
  avatar = 'S',
  signedIn = true,
  onNav,
  onAccount,
}: {
  dark: boolean;
  active?: string;
  avatar?: string;
  signedIn?: boolean;
  onNav?: (id: string) => void;
  onAccount?: () => void;
}) {
  return (
    <Glass
      dark={dark}
      radius={26}
      level="panel"
      style={{ width: 64, padding: '12px 0', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}
    >
      <div style={{ width: 40, height: 40, marginBottom: 8, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Cookie size={38} lobes={8} fill="var(--primary)">
          <span style={{ font: '800 19px/1 var(--font-sans)', color: 'var(--on-primary)' }}>T</span>
        </Cookie>
      </div>
      {NAV_ITEMS.map((it) => {
        const sel = it.id === active;
        return (
          <button className="tm-icon-btn"
            key={it.id}
            title={it.label}
            onClick={() => onNav?.(it.id)}
            style={{
              border: 'none',
              background: 'transparent',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              gap: 2,
              cursor: 'pointer',
              padding: '4px 0',
            }}
          >
            <div
              style={{
                width: 48,
                height: 32,
                borderRadius: 9999,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: sel ? 'var(--secondary-container)' : 'transparent',
              }}
            >
              <Icon
                name={it.icon}
                size={22}
                color={sel ? 'var(--on-secondary-container)' : 'var(--on-surface-variant)'}
                fill={sel}
                weight={500}
              />
            </div>
            <span
              style={{
                font: `${sel ? 600 : 500} 10px/1 var(--font-sans)`,
                color: sel ? 'var(--on-surface)' : 'var(--on-surface-variant)',
              }}
            >
              {it.label}
            </span>
          </button>
        );
      })}
      <div style={{ flex: 1 }} />
      <button className="tm-icon-btn" onClick={onAccount} title="Account" style={{ border: 'none', background: 'transparent', padding: 0, cursor: 'pointer' }}>
        {signedIn ? (
          <Cookie size={40} lobes={7} fill="var(--tertiary-container)">
            <span style={{ font: '700 15px/1 var(--font-sans)', color: 'var(--on-tertiary-container)' }}>{avatar}</span>
          </Cookie>
        ) : (
          <div
            style={{
              width: 44,
              height: 44,
              borderRadius: 9999,
              background: 'var(--surface-container-high)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <Icon name="login" size={22} color="var(--primary)" weight={500} />
          </div>
        )}
      </button>
    </Glass>
  );
}
