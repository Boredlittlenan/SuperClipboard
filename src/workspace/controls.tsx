import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { AlertCircle, ChevronDown, LoaderCircle } from 'lucide-react';
import { useLabels } from './hooks';

export function IconButton({ label, children, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return <button type="button" className="icon-button" aria-label={label} title={label} {...props}>{children}</button>;
}

export function EmptyState({ icon, title, message, action }: { icon: ReactNode; title: string; message?: string; action?: ReactNode }) {
  return <div className="empty-state">
    <div className="empty-icon">{icon}</div>
    <h3>{title}</h3>
    {message && <p>{message}</p>}
    {action}
  </div>;
}

export function LoadState({ error, retry }: { error?: string; retry: () => void }) {
  const { t } = useLabels();
  return error
    ? <EmptyState icon={<AlertCircle size={22}/>} title={t('加载失败', 'Could not load')} message={error}
      action={<button className="button" onClick={retry}>{t('重试', 'Retry')}</button>}/>
    : <div className="loading" role="status"><LoaderCircle size={17}/>{t('正在加载…', 'Loading…')}</div>;
}

export function Select({ value, onChange, children, label }: {
  value: string; onChange: (value: string) => void; children: ReactNode; label: string;
}) {
  return <div className="select-field"><select aria-label={label} value={value} onChange={(event) => onChange(event.target.value)}>
    {children}
  </select><ChevronDown size={15} aria-hidden="true"/></div>;
}
