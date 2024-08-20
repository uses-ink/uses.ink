import nextBuildId from "next-build-id";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url); // get the resolved path to the file
const __dirname = path.dirname(__filename); // get the name of the directory

import { readdirSync } from "node:fs";
import { join } from "node:path";
import { builtinModules } from "node:module";

const monorepoModules = (scope = undefined) => {
	const nodeModulesPath = join(__dirname, "node_modules");
	return readdirSync(scope ? join(nodeModulesPath, scope) : nodeModulesPath, {
		withFileTypes: true,
	})
		.filter((dirent) => dirent.isDirectory() && !dirent.name.startsWith("."))
		.map((dirent) => (scope ? `${scope}/${dirent.name}` : dirent.name))
		.reduce(
			(acc, name) =>
				undefined === scope && name.startsWith("@")
					? // biome-ignore lint/performance/noAccumulatingSpread: <explanation>
						[...acc, ...monorepoModules(name)]
					: // biome-ignore lint/performance/noAccumulatingSpread: <explanation>
						[...acc, name],
			[],
		);
};

/** @type {import('next').NextConfig} */
const nextConfig = {
	output: "standalone",
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
	transpilePackages: ["shiki", "astro-d2"],
};

export default nextConfig;
