export type ConnectionMode = 'url' | 'manual';

export const REMOTE_SETTING_KEYS = {
  storageMode: 'storage_mode',
  connectionMode: 'remote_db_connection_mode',
  url: 'remote_db_url',
  host: 'remote_db_host',
  port: 'remote_db_port',
  database: 'remote_db_database',
  username: 'remote_db_username',
  password: 'remote_db_password',
  sslMode: 'remote_db_ssl_mode',
  ready: 'remote_db_ready',
  profiles: 'remote_db_profiles',
} as const;

export type StoredSettingsPayload = Record<string, string>;

export interface RemoteDbProfile {
  id: string;
  name: string;
  connectionMode: ConnectionMode;
  url: string;
  host: string;
  port: string;
  database: string;
  username: string;
  password: string;
  sslMode: string;
  lastUsedAt: string;
}

function hashString(value: string): string {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

function withoutUrlPassword(value: string): string {
  try {
    const parsed = new URL(value);
    parsed.password = '';
    return parsed.toString();
  } catch {
    return value.replace(/:\/\/([^:@/]+):[^@/]*@/, '://$1@');
  }
}

function profileSignature(payload: StoredSettingsPayload): string {
  const mode = payload[REMOTE_SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url';
  const sslMode = payload[REMOTE_SETTING_KEYS.sslMode] || 'prefer';
  if (mode === 'manual') {
    return [
      mode,
      payload[REMOTE_SETTING_KEYS.host]?.trim().toLowerCase() ?? '',
      payload[REMOTE_SETTING_KEYS.port]?.trim() || '5432',
      payload[REMOTE_SETTING_KEYS.database]?.trim().toLowerCase() ?? '',
      payload[REMOTE_SETTING_KEYS.username]?.trim().toLowerCase() ?? '',
      sslMode,
    ].join('|');
  }
  return [mode, withoutUrlPassword(payload[REMOTE_SETTING_KEYS.url]?.trim() ?? '').toLowerCase(), sslMode].join('|');
}

function profileId(payload: StoredSettingsPayload): string {
  return `remote-${hashString(profileSignature(payload))}`;
}

export function profileLabel(payload: StoredSettingsPayload): string {
  const mode = payload[REMOTE_SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url';
  if (mode === 'manual') {
    const host = payload[REMOTE_SETTING_KEYS.host]?.trim();
    const port = payload[REMOTE_SETTING_KEYS.port]?.trim() || '5432';
    const database = payload[REMOTE_SETTING_KEYS.database]?.trim();
    const username = payload[REMOTE_SETTING_KEYS.username]?.trim();
    return [username && `${username}@`, host, host && `:${port}`, database && `/${database}`]
      .filter(Boolean)
      .join('') || 'PostgreSQL';
  }

  const rawUrl = payload[REMOTE_SETTING_KEYS.url]?.trim() ?? '';
  try {
    const parsed = new URL(rawUrl);
    const database = parsed.pathname.replace(/^\//, '');
    return [parsed.username && `${parsed.username}@`, parsed.host, database && `/${database}`]
      .filter(Boolean)
      .join('') || parsed.host || 'PostgreSQL';
  } catch {
    return withoutUrlPassword(rawUrl) || 'PostgreSQL';
  }
}

export function profileFromPayload(payload: StoredSettingsPayload, existingId?: string): RemoteDbProfile {
  return {
    id: existingId || profileId(payload),
    name: profileLabel(payload),
    connectionMode: payload[REMOTE_SETTING_KEYS.connectionMode] === 'manual' ? 'manual' : 'url',
    url: payload[REMOTE_SETTING_KEYS.url] ?? '',
    host: payload[REMOTE_SETTING_KEYS.host] ?? '',
    port: payload[REMOTE_SETTING_KEYS.port] || '5432',
    database: payload[REMOTE_SETTING_KEYS.database] ?? '',
    username: payload[REMOTE_SETTING_KEYS.username] ?? '',
    password: payload[REMOTE_SETTING_KEYS.password] ?? '',
    sslMode: payload[REMOTE_SETTING_KEYS.sslMode] || 'prefer',
    lastUsedAt: new Date().toISOString(),
  };
}

export function payloadFromProfile(profile: RemoteDbProfile, ready: boolean): StoredSettingsPayload {
  return {
    [REMOTE_SETTING_KEYS.storageMode]: 'remote',
    [REMOTE_SETTING_KEYS.connectionMode]: profile.connectionMode,
    [REMOTE_SETTING_KEYS.url]: profile.url,
    [REMOTE_SETTING_KEYS.host]: profile.host,
    [REMOTE_SETTING_KEYS.port]: profile.port || '5432',
    [REMOTE_SETTING_KEYS.database]: profile.database,
    [REMOTE_SETTING_KEYS.username]: profile.username,
    [REMOTE_SETTING_KEYS.password]: profile.password,
    [REMOTE_SETTING_KEYS.sslMode]: profile.sslMode || 'prefer',
    [REMOTE_SETTING_KEYS.ready]: ready ? 'true' : 'false',
  };
}

export function parseProfiles(value: string | undefined): RemoteDbProfile[] {
  if (!value) return [];
  try {
    const parsed = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map((item): RemoteDbProfile | null => {
        if (!item || typeof item !== 'object') return null;
        const profile = item as Partial<RemoteDbProfile>;
        const payload = payloadFromProfile({
          id: typeof profile.id === 'string' ? profile.id : '',
          name: typeof profile.name === 'string' ? profile.name : '',
          connectionMode: profile.connectionMode === 'manual' ? 'manual' : 'url',
          url: typeof profile.url === 'string' ? profile.url : '',
          host: typeof profile.host === 'string' ? profile.host : '',
          port: typeof profile.port === 'string' ? profile.port : '5432',
          database: typeof profile.database === 'string' ? profile.database : '',
          username: typeof profile.username === 'string' ? profile.username : '',
          password: typeof profile.password === 'string' ? profile.password : '',
          sslMode: typeof profile.sslMode === 'string' ? profile.sslMode : 'prefer',
          lastUsedAt: typeof profile.lastUsedAt === 'string' ? profile.lastUsedAt : '',
        }, false);
        const id = profile.id || profileId(payload);
        return {
          ...profileFromPayload(payload, id),
          name: profile.name || profileLabel(payload),
          lastUsedAt: profile.lastUsedAt || '',
        };
      })
      .filter((profile): profile is RemoteDbProfile => Boolean(profile))
      .slice(0, 12);
  } catch {
    return [];
  }
}

export function upsertProfile(profiles: RemoteDbProfile[], nextProfile: RemoteDbProfile): RemoteDbProfile[] {
  const nextSignature = profileSignature(payloadFromProfile(nextProfile, false));
  return [
    nextProfile,
    ...profiles.filter((profile) => (
      profile.id !== nextProfile.id
      && profileSignature(payloadFromProfile(profile, false)) !== nextSignature
    )),
  ].slice(0, 12);
}

export function profileIdForPayload(profiles: RemoteDbProfile[], payload: StoredSettingsPayload): string {
  const signature = profileSignature(payload);
  return profiles.find((profile) => profileSignature(payloadFromProfile(profile, false)) === signature)?.id
    || profileId(payload);
}

export function formatProfileTime(value: string): string {
  if (!value) return '';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  return `${month}/${day} ${hours}:${minutes}`;
}
