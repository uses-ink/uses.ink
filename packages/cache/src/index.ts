import { xxh64 } from "@node-rs/xxhash";
import { Redis } from "ioredis";
import { pack } from "msgpackr";
import { logger } from "@uses.ink/server-logger";
import { REDIS_URL } from "astro:env/server";

const cache: { current: Redis | null; hasWarned: boolean } = {
	current: null,
	hasWarned: false,
};

const makeCache = async () => {
	logger.debug("makeCache");
	if (!REDIS_URL) {
		if (!cache.hasWarned) {
			cache.hasWarned = true;
			logger.warn("REDIS_URL is not set");
		}
		return null;
	}
	const redis = new Redis(REDIS_URL);

	return redis;
};

export const getCache = async () => {
	if (cache.current === null) {
		cache.current = await makeCache();
	}
	return cache.current;
};

export type CacheType = "content" | "commit" | "tree" | "render";

export * from "./markdown";
export * from "./d2";
export * from "./github";
export * from "./typst";

export const hashObject = (obj: Record<string, unknown>) =>
	xxh64(pack(obj)).toString(36);
