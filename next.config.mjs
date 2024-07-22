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
};

export default nextConfig;
