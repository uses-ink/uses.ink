import { defineConfig } from "astro/config";
import tailwind from "@astrojs/tailwind";
import react from "@astrojs/react";
import { execSync } from "node:child_process";
import node from "@astrojs/node";

const commitHash = execSync("git rev-parse --short HEAD").toString().trim();
const version = execSync("git describe --tags").toString().trim();
const buildTime = new Date().toISOString();

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
		define: {
			__COMMIT_HASH__: JSON.stringify(commitHash),
			__VERSION__: JSON.stringify(version),
			__BUILD_TIME__: JSON.stringify(buildTime),
		},
	},
});

console.log(
	`Version: ${version}, Commit: ${commitHash}, Build Time: ${buildTime}`,
);

// import.meta.env = {
// 	...import.meta.env,
// 	PUBLIC_BUILD_COMMIT: commitHash,
// 	PUBLIC_BUILD_VERSION: version,
// 	PUBLIC_BUILD_TIME: buildTime,
// };
