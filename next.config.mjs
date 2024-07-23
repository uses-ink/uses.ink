import nextBuildId from "next-build-id";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url); // get the resolved path to the file
const __dirname = path.dirname(__filename); // get the name of the directory

/** @type {import('next').NextConfig} */
const nextConfig = {
	output: "standalone",
	webpack: (config, { buildId, dev, isServer, defaultLoaders, webpack }) => {
		config.module.rules.push({
			test: /\.svg$/,
			use: ["@svgr/webpack"],
		});
		// Important: return the modified config
		return config;
	},
	env: {
		NEXT_PUBLIC_BUILD_ID: `${nextBuildId.sync({ dir: __dirname })}@@${new Date().toISOString()}`,
	},
};

export default nextConfig;
