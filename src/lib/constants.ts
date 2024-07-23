export const DEFAULT_REPO = "blog";

export const README_FILES = ["README.md", "index.md"];

export const CACHE_TTL = 60; // 60 seconds
export const SHOW_DEV_TOOLS = false;

export const CONFIG_FILE = "uses.ink.json";

const buildIdParts = process.env.NEXT_PUBLIC_BUILD_ID?.split("@@");

export const BUILD_HASH = buildIdParts?.[0] ?? "dev";
export const BUILD_DATE = buildIdParts?.[1] ?? "Unknown";
