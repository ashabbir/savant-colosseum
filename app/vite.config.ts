import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

export default defineConfig({
  base: './',
  plugins: [react(), tailwindcss()],
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
  optimizeDeps: {
    exclude: ['lucide-react'],
  },
  server: {
    host: '127.0.0.1',
    port: 5175,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8090',
        changeOrigin: true,
      },
    },
  },
  // @ts-ignore
  test: {
    coverage: {
      provider: 'v8',
      include: ['src/renderer/services/**/*', 'src/renderer/components/**/*'],
      exclude: ['src/renderer/components/editors/**', 'src/renderer/components/modals/**'],
    },
  },
});
