import { xxh64 } from "@node-rs/xxhash";
import { Redis } from "ioredis";
import { pack } from "msgpackr";
import { serverLogger } from "@uses.ink/logger";

const cache: { current: Redis | null } = {
	current: null,
};

const makeCache = async () => {
	serverLogger.debug("makeCache");
	if (!process.env.REDIS_URL) {
		throw new Error("REDIS_URL is not defined");
	}
	const redis = new Redis(process.env.REDIS_URL);

	return redis;
};

export const getCache = async () => {
	if (cache.current === null) {
		cache.current = await makeCache();
	}
	return cache.current;
};

export type CacheType = "content" | "commit" | "tree" | "render";

export * from "./compile";
export * from "./d2";
export * from "./github";
export * from "./typst";

export const hashObject = (obj: Record<string, unknown>) =>
	xxh64(pack(obj)).toString(36);
