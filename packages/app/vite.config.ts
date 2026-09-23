import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		// The example specs live in the repo's fixtures/ and the wasm in the
		// engine package, both outside this package.
		fs: { allow: ['../..'] }
	},
	ssr: { noExternal: ['@threlte/core', '@threlte/extras', 'three'] }
});
