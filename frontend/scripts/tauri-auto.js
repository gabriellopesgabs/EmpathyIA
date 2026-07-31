#!/usr/bin/env node
/**
 * Auto-detect GPU and run Tauri with appropriate features.
 * Supports three commands:
 *   dev      – full dev mode (Tauri starts Next.js via beforeDevCommand)
 *   dev-only – backend-only (Next.js already running; clears beforeDevCommand)
 *   build    – production build
 */

const { execSync, spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// Get the command
const command = process.argv[2];
if (!command || !['dev', 'dev-only', 'build'].includes(command)) {
  console.error('Usage: node tauri-auto.js [dev|dev-only|build]');
  process.exit(1);
}

// Detect GPU feature
let feature = '';
if (process.env.TAURI_GPU_FEATURE) {
  feature = process.env.TAURI_GPU_FEATURE;
  console.log(`🔧 Using forced GPU feature from environment: ${feature}`);
} else {
  try {
    const result = execSync('node scripts/auto-detect-gpu.js', {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'inherit']
    });
    feature = result.trim();
  } catch (err) {
    // Detection failed – continue with no features
  }
}

console.log('');

// Platform-specific env vars
const platform = os.platform();
const env = { ...process.env };

if (platform === 'linux' && feature === 'cuda') {
  console.log('🐧 Linux/CUDA detected: Setting CMAKE flags for NVIDIA GPU');
  env.CMAKE_CUDA_ARCHITECTURES = '75';
  env.CMAKE_CUDA_STANDARD = '17';
  env.CMAKE_POSITION_INDEPENDENT_CODE = 'ON';
}

// ── dev-only: patch tauri.conf.json so Tauri won't launch a second Next.js ──
if (command === 'dev-only') {
  const confPath = path.join(__dirname, '..', 'src-tauri', 'tauri.conf.json');
  const conf = JSON.parse(fs.readFileSync(confPath, 'utf8'));
  const originalCmd = conf.build.beforeDevCommand;

  // Clear the beforeDevCommand so Tauri skips starting Next.js
  conf.build.beforeDevCommand = '';
  fs.writeFileSync(confPath, JSON.stringify(conf, null, 4));

  // Restore on exit
  const restore = () => {
    try {
      conf.build.beforeDevCommand = originalCmd;
      fs.writeFileSync(confPath, JSON.stringify(conf, null, 4));
    } catch (_) { }
  };
  process.on('exit', restore);
  process.on('SIGINT', () => { restore(); process.exit(0); });
  process.on('SIGTERM', () => { restore(); process.exit(0); });

  console.log('🚀 Running: tauri dev-only (Next.js already running on :3118)');
  if (feature && feature !== 'none') {
    console.log(`   Features: ${feature}`);
  }
  console.log('');

  // Map 'dev-only' → 'dev' for the actual tauri CLI call
  let tauriCmd = 'pnpm exec tauri dev';
  if (feature && feature !== 'none') tauriCmd += ` -- --features ${feature}`;

  try {
    execSync(tauriCmd, { stdio: 'inherit', env });
  } catch (err) {
    restore();
    process.exit(err.status || 1);
  }

  restore();
  process.exit(0);
}

// ── dev / build ──────────────────────────────────────────────────────────────
let tauriCmd = `npx @tauri-apps/cli ${command}`;
if (feature && feature !== 'none') {
  tauriCmd += ` --features ${feature}`;
  console.log(`🚀 Running: npx @tauri-apps/cli ${command} with features: ${feature}`);
} else {
  console.log(`🚀 Running: npx @tauri-apps/cli ${command} (CPU-only mode)`);
}
console.log('');

try {
  execSync(tauriCmd, { stdio: 'inherit', env });
} catch (err) {
  process.exit(err.status || 1);
}
