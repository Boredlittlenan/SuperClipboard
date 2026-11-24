import type { ReactNode } from 'react';

interface SettingRowProps {
  label: ReactNode;
  title?: string;
  children: ReactNode;
}

interface ToggleSettingRowProps {
  label: ReactNode;
  title?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: () => void | Promise<void>;
}

export function SettingRow({ label, title, children }: SettingRowProps) {
  return (
    <div style={styles.row} title={title}>
      <span style={styles.label}>{label}</span>
      {children}
    </div>
  );
}

export function ToggleSettingRow({
  label,
  title,
  checked,
  disabled = false,
  onChange,
}: ToggleSettingRowProps) {
  return (
    <SettingRow label={label} title={title}>
      <button
        type="button"
        className="settings-toggle"
        style={{
          ...styles.toggle,
          ...(checked ? styles.toggleOn : {}),
          ...(disabled ? styles.toggleDisabled : {}),
        }}
        aria-pressed={checked}
        disabled={disabled}
        onClick={() => void onChange()}
      >
        <span style={{ ...styles.knob, ...(checked ? styles.knobOn : {}) }} />
      </button>
    </SettingRow>
  );
}

const styles: Record<string, React.CSSProperties> = {
  row: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: '4px 0',
  },
  label: {
    fontSize: '12px',
    color: 'var(--text-primary)',
    flex: 1,
  },
  toggle: {
    position: 'relative',
    width: '34px',
    height: '18px',
    border: 'none',
    borderRadius: '9px',
    background: 'var(--border)',
    cursor: 'pointer',
    padding: 0,
    transition: 'background 0.2s',
    flexShrink: 0,
  },
  toggleOn: {
    background: 'var(--accent)',
  },
  toggleDisabled: {
    cursor: 'not-allowed',
    opacity: 0.55,
  },
  knob: {
    position: 'absolute',
    top: '2px',
    left: '2px',
    width: '14px',
    height: '14px',
    borderRadius: '50%',
    background: '#ffffff',
    transition: 'transform 0.2s',
  },
  knobOn: {
    transform: 'translateX(16px)',
  },
};
