export const DEFAULT_REPO = "blog";
export const DEFAULT_REF = "HEAD";

export const EXTENSIONS = ["md", "markdown"];

export const README_FILES = ["README", "index", "readme", "INDEX"].reduce(
	// biome-ignore lint/performance/noAccumulatingSpread: <explanation>
	(acc, file) => [...acc, ...EXTENSIONS.map((ext) => `${file}.${ext}`)],
	[] as string[],
);

export const GITHUB_CACHE_TTL = 60; // 60 seconds
export const RENDER_CACHE_TTL = 60 * 60 * 24; // 24 hours

export const SHOW_DEV_TOOLS = false;

export const CONFIG_FILE = "uses.ink.json";

export const IS_DEV = process.env.NODE_ENV === "development";
