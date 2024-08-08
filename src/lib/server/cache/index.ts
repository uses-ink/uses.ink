import { Redis } from "ioredis";
import { serverLogger } from "../logger";

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

export { getGitHubCache, setGitHubCache } from "./github";
export { getCompileCache, setCompileCache } from "./render";
