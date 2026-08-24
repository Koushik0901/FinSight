declare module "@tauri-apps/api/event" {
  export function listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void>;
  export function emit<T>(event: string, payload?: T): Promise<void>;
}
declare module "@tauri-apps/api/core" {
  export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
}
