import type { CSSProperties, ReactNode } from 'react';
import { Glass } from './Glass';
import { Icon } from './Icon';

/** Desktop detail / list side panel — the surface every entity detail + editor
 *  renders into. Ported from the design's `SidePanel`. */
export function SidePanel({
  dark,
  width = '100%',
  title,
  subtitle,
  onClose,
  headerExtra,
  footer,
  children,
}: {
  dark: boolean;
  width?: number | string;
  title?: ReactNode;
  subtitle?: ReactNode;
  onClose?: () => void;
  headerExtra?: ReactNode;
  footer?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <Glass
      dark={dark}
      level="panel"
      radius={28}
      style={{ width, maxHeight: '100%', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}
    >
      {title != null && (
        <div style={{ padding: '20px 20px 12px', display: 'flex', alignItems: 'flex-start', gap: 12, flexShrink: 0 }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ font: '700 22px/26px var(--font-sans)', letterSpacing: -0.4, color: 'var(--on-surface)' }}>{title}</div>
            {subtitle && (
              <div style={{ font: '400 13px/18px var(--font-sans)', color: 'var(--on-surface-variant)', marginTop: 3 }}>
                {subtitle}
              </div>
            )}
          </div>
          {headerExtra}
          {onClose && (
            <button
              onClick={onClose}
              style={{
                width: 38,
                height: 38,
                borderRadius: 9999,
                border: 'none',
                cursor: 'pointer',
                flexShrink: 0,
                background: 'var(--surface-container-highest)',
                color: 'var(--on-surface-variant)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <Icon name="close" size={20} color="var(--on-surface-variant)" />
            </button>
          )}
        </div>
      )}
      <div style={{ flex: 1, minHeight: 0, overflowY: 'auto', padding: '0 20px' }}>{children}</div>
      {footer && <div style={{ padding: 16, flexShrink: 0, borderTop: '1px solid var(--outline-variant)' }}>{footer}</div>}
    </Glass>
  );
}

export function Eyebrow({ children, style }: { children: ReactNode; style?: CSSProperties }) {
  return (
    <div
      style={{
        font: '700 11px/16px var(--font-sans)',
        letterSpacing: 1.4,
        textTransform: 'uppercase',
        color: 'var(--on-surface-variant)',
        ...style,
      }}
    >
      {children}
    </div>
  );
}

export function StatTile({ icon, value, label }: { icon?: string; value: string; label: string }) {
  return (
    <div
      style={{
        flex: 1,
        background: 'var(--surface-container-low)',
        borderRadius: 18,
        padding: '14px 12px',
        textAlign: 'center',
        minWidth: 0,
      }}
    >
      {icon && <Icon name={icon} size={20} color="var(--primary)" weight={500} />}
      <div style={{ font: '700 20px/24px var(--font-sans)', color: 'var(--on-surface)', marginTop: icon ? 5 : 0, letterSpacing: -0.3 }}>
        {value}
      </div>
      <div style={{ font: '500 11px/14px var(--font-sans)', color: 'var(--on-surface-variant)', marginTop: 2 }}>{label}</div>
    </div>
  );
}

export function Chip2({
  label,
  icon,
  selected,
  onClick,
}: {
  label: string;
  icon?: string;
  selected?: boolean;
  onClick?: () => void;
}) {
  const bg = selected ? 'var(--secondary-container)' : 'transparent';
  const fg = selected ? 'var(--on-secondary-container)' : 'var(--on-surface-variant)';
  return (
    <button
      onClick={onClick}
      style={{
        height: 34,
        padding: icon ? '0 14px 0 10px' : '0 14px',
        borderRadius: 10,
        background: bg,
        color: fg,
        border: selected ? 'none' : '1px solid var(--outline-variant)',
        display: 'inline-flex',
        alignItems: 'center',
        gap: 6,
        cursor: 'pointer',
        font: '600 13px/1 var(--font-sans)',
        whiteSpace: 'nowrap',
      }}
    >
      {selected && <Icon name="check" size={16} color={fg} weight={600} />}
      {icon && !selected && <Icon name={icon} size={16} color={fg} weight={500} />}
      {label}
    </button>
  );
}

/** Solid secondary-container icon button (detail action row). */
export function GlassIconBtnSolid({ icon, onClick, title }: { icon: string; onClick?: () => void; title?: string }) {
  return (
    <button
      title={title}
      onClick={onClick}
      style={{
        width: 48,
        height: 48,
        border: 'none',
        cursor: 'pointer',
        background: 'var(--secondary-container)',
        color: 'var(--on-secondary-container)',
        borderRadius: 16,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        flexShrink: 0,
      }}
    >
      <Icon name={icon} size={22} color="var(--on-secondary-container)" weight={500} />
    </button>
  );
}

/** Expressive connected segmented control (mode / profile tabs). */
export function Tabs({
  items,
  active,
  onPick,
}: {
  items: { label: string; icon?: string }[];
  active: number;
  onPick: (i: number) => void;
}) {
  return (
    <div style={{ display: 'flex', gap: 4 }}>
      {items.map((it, i) => {
        const sel = i === active;
        return (
          <button
            key={it.label}
            onClick={() => onPick(i)}
            style={{
              flex: 1,
              height: 40,
              border: 'none',
              cursor: 'pointer',
              background: sel ? 'var(--primary)' : 'var(--surface-container-high)',
              color: sel ? 'var(--on-primary)' : 'var(--on-surface-variant)',
              borderRadius: sel ? 9999 : 10,
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: 7,
              font: '600 13px/1 var(--font-sans)',
              letterSpacing: 0.1,
              transition: 'border-radius .25s var(--ease-out), background .2s',
            }}
          >
            {it.icon && <Icon name={it.icon} size={17} color={sel ? 'var(--on-primary)' : 'var(--on-surface-variant)'} fill={sel} weight={500} />}
            {it.label}
          </button>
        );
      })}
    </div>
  );
}

/** M3 filled text field (label + underline), used in the marker editor. */
export function FilledField({
  label,
  value,
  onChange,
  placeholder,
  multiline,
  type = 'text',
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  multiline?: boolean;
  type?: 'text' | 'password' | 'email';
}) {
  const common: CSSProperties = {
    width: '100%',
    border: 'none',
    outline: 'none',
    background: 'transparent',
    font: '400 16px/22px var(--font-sans)',
    color: 'var(--on-surface)',
    resize: 'none',
  };
  return (
    <div
      style={{
        background: 'var(--surface-container-high)',
        borderRadius: '12px 12px 0 0',
        borderBottom: '2px solid var(--primary)',
        padding: '10px 14px 8px',
      }}
    >
      <div style={{ font: '600 11px/13px var(--font-sans)', color: 'var(--primary)' }}>{label}</div>
      {multiline ? (
        <textarea value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} rows={3} style={{ ...common, marginTop: 4 }} />
      ) : (
        <input type={type} value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} style={{ ...common, marginTop: 2 }} />
      )}
    </div>
  );
}
