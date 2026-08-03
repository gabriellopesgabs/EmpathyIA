import { getVersion } from '@tauri-apps/api/app';
import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export interface UpdateInfo {
  available: boolean;
  currentVersion: string;
  version?: string;
  date?: string;
  body?: string;
}

export interface UpdateProgress {
  downloaded: number;
  total: number;
  percentage: number;
}

const LAST_CHECK_KEY = 'empathy:last-update-check';
const CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

class UpdateService {
  private pendingUpdate: Update | null = null;
  private checking = false;

  async checkForUpdates(force = false): Promise<UpdateInfo> {
    const currentVersion = await getVersion();

    if (!force && this.wasCheckedRecently()) {
      return { available: false, currentVersion };
    }

    if (this.checking) {
      return { available: false, currentVersion };
    }

    this.checking = true;
    try {
      const update = await check();
      this.pendingUpdate = update;
      window.localStorage.setItem(LAST_CHECK_KEY, Date.now().toString());

      if (!update) {
        return { available: false, currentVersion };
      }

      return {
        available: true,
        currentVersion,
        version: update.version,
        date: update.date,
        body: update.body,
      };
    } finally {
      this.checking = false;
    }
  }

  async downloadAndInstall(onProgress: (progress: UpdateProgress) => void): Promise<void> {
    const update = this.pendingUpdate ?? await check();
    if (!update) {
      throw new Error('A atualização não está mais disponível.');
    }

    let downloaded = 0;
    let total = 0;

    await update.downloadAndInstall((event: DownloadEvent) => {
      if (event.event === 'Started') {
        total = event.data.contentLength ?? 0;
        onProgress({ downloaded: 0, total, percentage: 0 });
      } else if (event.event === 'Progress') {
        downloaded += event.data.chunkLength;
        onProgress({
          downloaded,
          total,
          percentage: total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0,
        });
      } else if (event.event === 'Finished') {
        onProgress({ downloaded: total || downloaded, total, percentage: 100 });
      }
    });

    await relaunch();
  }

  private wasCheckedRecently(): boolean {
    const value = window.localStorage.getItem(LAST_CHECK_KEY);
    if (!value) return false;
    const timestamp = Number(value);
    return Number.isFinite(timestamp) && Date.now() - timestamp < CHECK_INTERVAL_MS;
  }
}

export const updateService = new UpdateService();
