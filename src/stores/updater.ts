import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { tauriError } from "../lib/tauri";

/** `available` is the only state that renders the banner / the header's
 *  "Actualizar" button; `checking`/`installing` are transient. */
export type UpdaterStatus = "idle" | "checking" | "available" | "installing" | "error";

interface UpdaterState {
  status: UpdaterStatus;
  /** The update the last `check()` found — carries `downloadAndInstall()`. */
  update: Update | null;
  /** Only set by a MANUAL check: a startup check must never make the app
   *  look broken just because there is no release yet or the machine is
   *  offline. */
  error: string | null;
  /** "Más tarde" on the banner: hides it for this session while the header
   *  button keeps offering the update. */
  dismissed: boolean;
  /** Called once when the app opens. */
  checkOnStartup: () => Promise<void>;
  /** Called from the header — the "menu" way to look for a new version. */
  checkNow: () => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

async function runCheck(
  set: (partial: Partial<UpdaterState>) => void,
  get: () => UpdaterState,
  manual: boolean,
): Promise<void> {
  const { status } = get();
  if (status === "checking" || status === "installing") return;
  set({ status: "checking", error: null });
  try {
    const update = await check();
    set(update ? { status: "available", update, dismissed: false } : { status: "idle", update: null, error: null });
  } catch (e) {
    if (manual) set({ status: "error", update: null, error: tauriError(e).message });
    else set({ status: "idle", update: null, error: null });
  }
}

export const useUpdater = create<UpdaterState>((set, get) => ({
  status: "idle",
  update: null,
  error: null,
  dismissed: false,

  checkOnStartup: () => runCheck(set, get, false),
  checkNow: () => runCheck(set, get, true),

  install: async () => {
    const { update } = get();
    if (!update || get().status === "installing") return;
    set({ status: "installing", error: null });
    try {
      await update.downloadAndInstall();
    } catch (e) {
      // Nothing was replaced — keep offering the update so it can be retried.
      set({ status: "available", error: tauriError(e).message });
      return;
    }
    // Windows hands over to the installer, which ends this process itself;
    // macOS/Linux need the explicit restart to run the new version.
    try {
      await relaunch();
    } catch {
      /* the process is already gone */
    }
  },

  dismiss: () => set({ dismissed: true }),
}));
