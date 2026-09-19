import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		// The example specs live in the repo's fixtures/ and the wasm in the
		// engine package, both outside this package.
		fs: { allow: ['../..'] }
	},
	ssr: { noExternal: ['@threlte/core', '@threlte/extras', 'three'] }
});
