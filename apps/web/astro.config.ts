import { defineConfig, envField } from "astro/config";
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
	experimental: {
		env: {
			schema: {
				GITHUB_TOKEN: envField.string({
					context: "server",
					access: "secret",
					optional: true,
				}),
				REDIS_URL: envField.string({
					context: "server",
					access: "secret",
					optional: true,
				}),
				RENDERER: envField.enum({
					access: "public",
					context: "server",
					values: ["mdx", "marked", "markdownIt"],
					default: "mdx",
				}),
				GITHUB_CACHE_TTL: envField.number({
					access: "public",
					context: "server",
					default: 60, // 60 seconds
				}),
				RENDER_CACHE_TTL: envField.number({
					access: "public",
					context: "server",
					default: 60 * 60 * 24, // 24 hours
				}),
				DEBUG_TREE: envField.boolean({
					access: "public",
					context: "server",
					default: false,
				}),
				DISABLE_CACHE_DEV: envField.boolean({
					access: "public",
					context: "server",
					default: true,
				}),
			},
			validateSecrets: true,
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
