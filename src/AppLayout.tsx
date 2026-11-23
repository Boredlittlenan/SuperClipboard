import { lazy, Suspense } from 'react';
import { useAppSettings } from './hooks/useAppSettings';

const Classic = lazy(() => import('./App'));
const Beta = lazy(() => import('./workspace/Workspace').then(({ Workspace }) => ({ default: Workspace })));

export default function AppLayout() {
  const { settings, ready } = useAppSettings();
  if (!ready) return null;
  return <Suspense fallback={null}>{settings.workspaceModeBeta ? <Beta /> : <Classic />}</Suspense>;
}
