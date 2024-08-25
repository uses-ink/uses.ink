export const DEFAULT_REPO = "blog";
export const DEFAULT_REF = "HEAD";

export const DEFAULT_META = {
	readingTime: true,
};
export const EXTENSIONS = ["md", "markdown", "typ"];

export const README_FILES = ["README", "index"].reduce(
	// biome-ignore lint/performance/noAccumulatingSpread: <explanation>
	(acc, file) => [...acc, ...EXTENSIONS.map((ext) => `${file}.${ext}`)],
	[] as string[],
);

export const CONFIG_FILE = "uses.ink.json";

export const IS_DEV = !!import.meta.env.DEV;

export const DEBUG_TREE = false;
export const DISABLE_CACHE_DEV = false;

export const ALLOWED_NODES = ["mdxjsEsm", "mdxJsxFlowElement"];

export const BADGES_HOSTS = ["img.shields.io", "badgen.net", "forthebadge.com"];

export const DARK_HASHES = [
	"#dark",
	"#dark-mode",
	"#darkmode",
	"#gh-dark-mode-only",
];
export const LIGHT_HASHES = [
	"#light",
	"#light-mode",
	"#lightmode",
	"#gh-light-mode-only",
];

export * from "./errors";
