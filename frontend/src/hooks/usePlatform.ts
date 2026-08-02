import { useState, useEffect } from 'react';

export type Platform = 'macos' | 'windows' | 'linux' | 'unknown';

/**
 * Detect platform from user agent (fallback method)
 */
function detectPlatformFromUserAgent(): Platform {
  if (typeof navigator === 'undefined') return 'unknown';

  const userAgent = navigator.userAgent.toLowerCase();
  if (userAgent.includes('mac')) {
    return 'macos';
  } else if (userAgent.includes('win')) {
    return 'windows';
  } else if (userAgent.includes('linux')) {
    return 'linux';
  }
  return 'unknown';
}

/**
 * Hook to detect the current platform
 * Uses the WebView user agent, avoiding an unnecessary privileged OS plugin.
 * @returns The current platform
 */
export function usePlatform(): Platform {
  const [currentPlatform, setCurrentPlatform] = useState<Platform>(() => detectPlatformFromUserAgent());

  useEffect(() => {
    setCurrentPlatform(detectPlatformFromUserAgent());
  }, []);

  return currentPlatform;
}

/**
 * Simple helper to check if the current platform is Linux
 * @returns true if running on Linux
 */
export function useIsLinux(): boolean {
  const currentPlatform = usePlatform();
  return currentPlatform === 'linux';
}
