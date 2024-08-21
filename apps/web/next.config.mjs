import nextBuildId from "next-build-id";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url); // get the resolved path to the file
const __dirname = path.dirname(__filename); // get the name of the directory

/** @type {import('next').NextConfig} */
const nextConfig = {
	output: "standalone",
	experimental: {
		serverComponentsExternalPackages: [
			"@myriaddreamin/typst-ts-node-compiler",
			"@node-rs/xxhash",
			"pikchr-wasm",
		],
	},
	webpack: (config, { buildId, dev, isServer, defaultLoaders, webpack }) => {
		config.module.rules.push({
			test: /\.svg$/,
			use: ["@svgr/webpack"],
		});
		// Gosh this is a mess, NextJS please fix this
		config.externals.push({
			"@myriaddreamin/typst-ts-node-compiler":
				"commonjs @myriaddreamin/typst-ts-node-compiler",
			"@node-rs/xxhash": "commonjs @node-rs/xxhash",
			"pikchr-wasm": "module pikchr-wasm",
		});
		// Important: return the modified config
		return config;
	},
	env: {
		NEXT_PUBLIC_BUILD_VERSION: nextBuildId.sync({
			dir: __dirname,
			describe: true,
		}),
		NEXT_PUBLIC_BUILD_COMMIT: nextBuildId.sync({
			dir: __dirname,
		}),
	},
	transpilePackages: [
		"shiki",
		"astro-d2",
		"@uses.ink/cache",
		"@uses.ink/render",
		"@uses.ink/types",
		"@uses.ink/constants",
		"@uses.ink/logger",
		"@uses.ink/errors",
	],
};

export default nextConfig;
