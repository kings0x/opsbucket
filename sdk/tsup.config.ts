import { defineConfig } from 'tsup'

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['cjs', 'esm'],
  dts: true,
  splitting: false,
  sourcemap: true,
  clean: true,
  minify: true,
  target: 'es2017',
  define: {
    '__SDK_VERSION__': JSON.stringify(process.env.npm_package_version),
  },
})
