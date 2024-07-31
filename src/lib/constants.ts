export const DEFAULT_REPO = "blog";
export const DEFAULT_BRANCH = "main";

export const EXTENSIONS = ["md", "markdown"];

export const README_FILES = ["README", "index"].reduce(
	// biome-ignore lint/performance/noAccumulatingSpread: <explanation>
	(acc, file) => [...acc, ...EXTENSIONS.map((ext) => `${file}.${ext}`)],
	[] as string[],
);

export const CACHE_TTL = 60; // 60 seconds
export const SHOW_DEV_TOOLS = false;

export const CONFIG_FILE = "uses.ink.json";

export const IS_DEV = process.env.NODE_ENV === "development";
