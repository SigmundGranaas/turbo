import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useSession, loginWithPassword, registerWithPassword, loginWithGoogle, logout } from '../../api/auth';
import { getProfile } from '../../api/sharing';
import { ApiError } from '../../api/client';
import { useUiStore, type ThemeMode, type Units } from '../../store/uiStore';
import { SidePanel, Eyebrow, Tabs, FilledField } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';
import { Cookie } from '../../ui/Cookie';
import { FriendsSection } from './FriendsSection';

const THEME_ORDER: ThemeMode[] = ['system', 'light', 'dark'];

/** My-position dot palette — the shared track palette so colour pickers read
 *  the same across the app; the default blue is the separate first swatch. */
const DOT_COLORS = ['#C75B39', '#059669', '#7C3AED', '#DB2777', '#D97706', '#0891B2', '#475569'];

/** Account + settings side panel. Account: signed-in identity / sign-out, or an
 *  email-password + Google sign-in form. Settings: theme + units. Ports the
 *  design's SignIn + AccountSettings. */
export function AccountSettingsPanel({ dark, onClose }: { dark: boolean; onClose: () => void }) {
  const session = useSession();
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, enabled: Boolean(session.data), staleTime: 60_000 });
  const theme = useUiStore((s) => s.theme);
  const units = useUiStore((s) => s.units);
  const distanceHaze = useUiStore((s) => s.distanceHaze);
  const locationDotColor = useUiStore((s) => s.locationDotColor);

  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setError(null);
    if (mode === 'register' && password !== confirm) {
      setError('Passwords don’t match.');
      return;
    }
    setBusy(true);
    try {
      if (mode === 'login') await loginWithPassword(email, password);
      else await registerWithPassword(email, password, confirm);
      await session.refetch();
    } catch (e) {
      setError(e instanceof ApiError && e.status === 401 ? 'Wrong email or password.' : 'Sign-in failed. Please try again.');
    } finally {
      setBusy(false);
    }
  };

  const signedIn = Boolean(session.data);
  const who = session.data?.name ?? session.data?.email ?? '';

  return (
    <SidePanel dark={dark} title="Account" onClose={onClose}>
      <div style={{ paddingTop: 8, paddingBottom: 16 }}>
        {signedIn ? (
          <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
            <Cookie size={52} lobes={8} fill="var(--primary)">
              <span style={{ font: '700 20px/1 var(--font-sans)', color: 'var(--on-primary)' }}>{who.trim().charAt(0).toUpperCase() || 'S'}</span>
            </Cookie>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ font: '600 16px/20px var(--font-sans)', color: 'var(--on-surface)' }}>{session.data?.name ?? 'Signed in'}</div>
              <div style={{ font: '400 13px/18px var(--font-sans)', color: 'var(--on-surface-variant)', overflow: 'hidden', textOverflow: 'ellipsis' }}>{session.data?.email}</div>
              {profile.data?.friendCode && (
                <div style={{ font: '500 12px/16px var(--font-sans)', color: 'var(--primary)', marginTop: 3, fontVariantNumeric: 'tabular-nums' }}>
                  Friend code · {profile.data.friendCode}
                </div>
              )}
            </div>
            <Btn label="Sign out" tone="surface" onClick={() => void logout().then(() => session.refetch())} />
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <FilledField label="Email" value={email} onChange={setEmail} placeholder="you@example.com" type="email" />
            <FilledField label="Password" value={password} onChange={setPassword} type="password" />
            {mode === 'register' && <FilledField label="Confirm password" value={confirm} onChange={setConfirm} type="password" />}
            {error && <div style={{ font: '400 13px/18px var(--font-sans)', color: 'var(--error)' }}>{error}</div>}
            <Btn label={busy ? 'Please wait…' : mode === 'login' ? 'Sign in' : 'Create account'} full onClick={busy ? undefined : submit} />
            <button
              onClick={() => void loginWithGoogle()}
              style={{
                height: 48,
                border: '1px solid var(--outline-variant)',
                background: 'var(--surface-container-low)',
                color: 'var(--on-surface)',
                borderRadius: 9999,
                cursor: 'pointer',
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 10,
                font: '600 15px/1 var(--font-sans)',
              }}
            >
              <Icon name="login" size={20} color="var(--primary)" weight={500} /> Continue with Google
            </button>
            <button
              onClick={() => {
                setMode(mode === 'login' ? 'register' : 'login');
                setError(null);
              }}
              style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--primary)', font: '600 13px/1 var(--font-sans)', padding: 4 }}
            >
              {mode === 'login' ? 'New here? Create an account' : 'Have an account? Sign in'}
            </button>
          </div>
        )}

        {signedIn && <FriendsSection />}

        <Eyebrow style={{ margin: '24px 0 10px' }}>Appearance</Eyebrow>
        <Tabs
          items={[{ label: 'System' }, { label: 'Light' }, { label: 'Dark' }]}
          active={Math.max(0, THEME_ORDER.indexOf(theme))}
          onPick={(i) => useUiStore.getState().setTheme(THEME_ORDER[i])}
        />

        <Eyebrow style={{ margin: '20px 0 10px' }}>Units</Eyebrow>
        <Tabs
          items={[{ label: 'Metric' }, { label: 'Imperial' }]}
          active={units === 'metric' ? 0 : 1}
          onPick={(i) => useUiStore.getState().setUnits((i === 0 ? 'metric' : 'imperial') as Units)}
        />

        <Eyebrow style={{ margin: '20px 0 10px' }}>3D map</Eyebrow>
        <Tabs
          items={[{ label: 'Off' }, { label: 'On' }]}
          active={distanceHaze ? 1 : 0}
          onPick={(i) => useUiStore.getState().setDistanceHaze(i === 1)}
        />
        <div style={{ margin: '8px 2px 0', font: '400 12px/17px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
          Distance haze — distant terrain takes on a faint atmospheric colour when
          tilted toward the horizon.
        </div>

        <Eyebrow style={{ margin: '20px 0 10px' }}>Location dot</Eyebrow>
        <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
          {[undefined, ...DOT_COLORS].map((c) => {
            const sel = (c ?? null) === (locationDotColor ?? null);
            return (
              <button
                key={c ?? 'default'}
                title={c ?? 'Default blue'}
                onClick={() => useUiStore.getState().setLocationDotColor(c)}
                style={{
                  width: 30,
                  height: 30,
                  borderRadius: '50%',
                  cursor: 'pointer',
                  background: c ?? '#2563EB',
                  border: sel ? '3px solid var(--on-surface)' : '3px solid transparent',
                  boxShadow: '0 1px 3px rgba(0,0,0,.25)',
                }}
              />
            );
          })}
        </div>

        <Eyebrow style={{ margin: '24px 0 8px' }}>About</Eyebrow>
        <div style={{ font: '400 13px/19px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
          Turbo for Web · the turbomap wgpu renderer in the browser.
        </div>
      </div>
    </SidePanel>
  );
}
