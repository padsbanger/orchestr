import { confirm } from "@tauri-apps/plugin-dialog";

type DestructiveActionOptions = {
  title: string;
  message: string;
  confirmLabel: string;
};

export async function runConfirmedDestructiveAction(
  options: DestructiveActionOptions,
  action: () => Promise<void>,
): Promise<boolean> {
  const confirmed = await confirm(options.message, {
    title: options.title,
    kind: "warning",
    okLabel: options.confirmLabel,
    cancelLabel: "Cancel",
  });

  if (!confirmed) return false;
  await action();
  return true;
}

export function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (typeof error === "object" && error && "message" in error && typeof error.message === "string") return error.message;
  return fallback;
}
