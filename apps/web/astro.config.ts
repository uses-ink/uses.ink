import { defineConfig } from "astro/config";
import tailwind from "@astrojs/tailwind";
import react from "@astrojs/react";

import node from "@astrojs/node";

// https://astro.build/config
export default defineConfig({
	integrations: [
		tailwind({
			applyBaseStyles: false,
		}),
		react(),
	],
	output: "server",
	adapter: node({
		mode: "standalone",
	}),
	vite: {
		ssr: {
			external: ["@node-rs/xxhash", "@myriaddreamin/typst-ts-node-compiler"],
		},
	},
});
