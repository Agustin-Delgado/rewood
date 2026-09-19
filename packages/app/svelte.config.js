import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		// Client-only app: the engine runs as WASM in the browser.
		adapter: adapter({ fallback: 'index.html' })
	}
};

export default config;
