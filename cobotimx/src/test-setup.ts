// Node >= 22.5 exposes an experimental global `localStorage`/`sessionStorage`
// whose getter returns undefined unless --experimental-webstorage storage is
// configured. Vitest's jsdom environment then skips copying jsdom's storage
// onto globalThis (the name already exists), leaving `localStorage` undefined
// in tests. Install a minimal in-memory Storage when that happens.
class MemoryStorage {
  #data = new Map<string, string>();

  get length(): number {
    return this.#data.size;
  }

  clear(): void {
    this.#data.clear();
  }

  getItem(key: string): string | null {
    return this.#data.has(key) ? this.#data.get(key)! : null;
  }

  key(index: number): string | null {
    return [...this.#data.keys()][index] ?? null;
  }

  removeItem(key: string): void {
    this.#data.delete(key);
  }

  setItem(key: string, value: string): void {
    this.#data.set(key, String(value));
  }
}

for (const key of ["localStorage", "sessionStorage"] as const) {
  let broken = false;
  try {
    broken = globalThis[key] == null;
  } catch {
    broken = true;
  }
  if (broken) {
    Object.defineProperty(globalThis, key, {
      value: new MemoryStorage(),
      configurable: true,
      writable: true,
    });
  }
}
