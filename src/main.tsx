import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import AppLayout from './AppLayout';
import AppSettingsProvider from './components/settings/AppSettingsProvider';
import { I18nProvider } from './i18n';
import './index.css';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <I18nProvider>
      <AppSettingsProvider><AppLayout /></AppSettingsProvider>
    </I18nProvider>
  </StrictMode>,
);
