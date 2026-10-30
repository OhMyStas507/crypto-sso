import { createSignal } from "solid-js";
import { command, type Command } from "./bridge";
import { useQueryClient } from "@tanstack/solid-query";

export function useAction() {
  const [busy, setBusy] = createSignal(false),
    [error, setError] = createSignal<unknown>();
  const client = useQueryClient();
  return {
    busy,
    error,
    clear: () => setError(undefined),
    async run<T>(name: Command, args: unknown = {}): Promise<T | undefined> {
      if (busy()) return;
      setBusy(true);
      setError(undefined);
      try {
        const result = await command<T>(name, args);
        await client.invalidateQueries();
        return result;
      } catch (error) {
        setError(error);
      } finally {
        setBusy(false);
      }
    },
  };
}
