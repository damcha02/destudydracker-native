import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.endsWith('/src/lib/wordleWords.ts')) return 'game-wordle-words'
          if (id.endsWith('/src/lib/countries.ts')) return 'game-countries'
          if (id.endsWith('/src/lib/countryBorders.ts')) return 'game-country-borders'
          if (id.endsWith('/src/lib/travleMapData.ts')) return 'game-travle-map'
          if (id.includes('/src/assets/flags/')) return 'game-flags'
          if (id.endsWith('/src/lib/flaggle.ts')) return 'game-flaggle'
          if (id.endsWith('/src/lib/travle.ts')) return 'game-travle'
          if (id.endsWith('/src/lib/wordle.ts')) return 'game-wordle'
          if (id.endsWith('/src/lib/geodle.ts')) return 'game-geodle'
          if (!id.includes('/node_modules/')) return undefined
          if (id.includes('/react/') || id.includes('/react-dom/')) return 'vendor-react'
          if (id.includes('/@tauri-apps/')) return 'vendor-tauri'
          if (id.includes('/pdfjs-dist/')) return 'vendor-pdf'
          return 'vendor'
        },
      },
    },
  },
})
