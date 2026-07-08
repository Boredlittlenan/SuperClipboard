export type AppEventName =
  | 'app:resume'
  | 'clipboard:changed'
  | 'storage:changed';

const appEventTarget = new EventTarget();

export function emitAppEvent(name: AppEventName): void {
  appEventTarget.dispatchEvent(new Event(name));
}

export function onAppEvent(name: AppEventName, handler: () => void): () => void {
  appEventTarget.addEventListener(name, handler);
  return () => appEventTarget.removeEventListener(name, handler);
}
