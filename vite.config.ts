import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  // Don't print the URL since Tauri handles the dev server
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Don't watch Rust files, avoid triggering reloads on Cargo rebuilds
      ignored: ['**/src-tauri/**'],
    },
  },
});
