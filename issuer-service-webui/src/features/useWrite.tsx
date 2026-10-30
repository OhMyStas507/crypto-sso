import { createSignal } from "solid-js";
import { api, operationKey } from "../api";

export function useWrite() {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<unknown>();
  let previous = "",
    key = "";
  return {
    busy,
    error,
    clear: () => setError(undefined),
    async run<T>(
      path: string,
      method: string,
      value: unknown,
    ): Promise<T | undefined> {
      if (busy()) return;
      const content = JSON.stringify([path, method, value]);
      if (content !== previous) {
        previous = content;
        key = operationKey();
      }
      setBusy(true);
      setError(undefined);
      try {
        const result = await api<T>(path, method, value, key);
        previous = "";
        return result;
      } catch (error) {
        setError(error);
      } finally {
        setBusy(false);
      }
    },
  };
}
