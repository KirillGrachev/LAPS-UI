/**
 * Конфигурация Vite для Tauri-фронтенда: не гасить вывод rust (clearScreen),
 * фиксированный порт 1420 — Tauri ждёт именно его и падает, если порт занят,
 * `src-tauri` не watchится.
 */
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import {defineConfig} from 'vite';
import pkg from './package.json';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    '__APP_VERSION__': JSON.stringify(pkg.version),
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || false,
    hmr: process.env.TAURI_DEV_HOST
      ? {
          protocol: 'ws',
          host: process.env.TAURI_DEV_HOST,
          port: 1421,
        }
      : process.env.DISABLE_HMR !== 'true',
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    sourcemap: false,
    cssCodeSplit: false,
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
        passes: 3,
      },
      mangle: true,
      format: {
        comments: false,
      },
    },
    chunkSizeWarningLimit: 1000,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules')) {
            if (
              id.includes('react') ||
              id.includes('react-dom') ||
              id.includes('scheduler') ||
              id.includes('motion') ||
              id.includes('lucide-react') ||
              id.includes('@radix-ui') ||
              id.includes('sonner')
            ) {
              return 'vendor-core';
            }
            if (id.includes('@tauri-apps')) {
              return 'vendor-tauri';
            }
            if (id.includes('date-fns') || id.includes('i18next')) {
              return 'vendor-misc';
            }
          }
        },
      },
    },
  },
});
