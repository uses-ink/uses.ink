import { createCache, type Store, type Cache } from "cache-manager";
import { redisStore } from "cache-manager-redis-yet";
import type { GitHubContent, GitHubRequest } from "./types";
import type { OctokitResponse } from "@octokit/types";
import { CACHE_TTL } from "./constants";

const cache: { current: Cache<Store> | null } = {
	current: null,
};

const makeCache = async () => {
	return createCache(await redisStore({ url: process.env.REDIS_URL }), {
		ttl: CACHE_TTL,
	});
};

export const getCache = async () => {
	if (cache.current === null) {
		cache.current = await makeCache();
	}
	return cache.current;
};

const getKey = (request: GitHubRequest, type: CacheType): string => {
	const { owner, path, repo } = request;
	const key = `${owner}/${repo}/${path}@${type}`;
	return key;
};

type CacheType = "content" | "commit";

export const getGitHubCache = async <R = GitHubContent>(
	request: GitHubRequest,
	type: "content" | "commit" = "content",
): Promise<OctokitResponse<R> | null> => {
	const cache = await getCache();
	if (cache === null) {
		console.log("cache is null");
		return null;
	}
	const key = getKey(request, type);

	const data = (await cache.get<null | OctokitResponse<R>>(key)) ?? null;
	return data;
};

export const setGitHubCache = async <R = GitHubContent>(
	request: GitHubRequest,
	response: OctokitResponse<R>,
	type: CacheType = "content",
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getKey(request, type);
	await cache.set(key, response);
};
