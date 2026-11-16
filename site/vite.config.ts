import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';

const local = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
	// GitHub Pages serves a project site under /<repo>/; the deploy workflow sets
	// BASE_PATH. Locally the site is served from the root.
	base: process.env.BASE_PATH ?? '/',
	resolve: {
		// Use the npm package straight from its source in js/, so the site always
		// matches the library. Its WASM module must be built first
		// (`npm run build:wasm` in js/).
		alias: [
			{ find: /^text-shape$/, replacement: local('../js/src/index.ts') },
			{
				find: /.*\/wasm\/text_shape_wasm_base64\.js$/,
				replacement: local('src/no-embedded-wasm.ts'),
			},
		],
	},
	server: {
		fs: { allow: ['.', '../js', '../fonts'] },
	},
	build: {
		target: 'es2022', // top-level await
	},
});
