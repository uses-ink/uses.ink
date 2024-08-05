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
			"rehype-mermaid",
		],
	},
	webpack: (config, { buildId, dev, isServer, defaultLoaders, webpack }) => {
		config.module.rules.push({
			test: /\.svg$/,
			use: ["@svgr/webpack"],
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
	transpilePackages: ["shiki"],
};

export default nextConfig;
