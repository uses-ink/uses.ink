import { createCache, type Store, type Cache } from "cache-manager";
import { redisStore } from "cache-manager-redis-yet";
import type { GitHubContent, GitHubRequest } from "./types";
import type { OctokitResponse } from "@octokit/types";

type GitHubResponse = OctokitResponse<GitHubContent>;

const cache: { current: Cache<Store> | null } = {
	current: null,
};

const makeCache = async () => {
	return createCache(await redisStore({ url: process.env.REDIS_URL }));
};

export const getCache = async () => {
	if (cache.current === null) {
		cache.current = await makeCache();
	}
	return cache.current;
};

const getKey = (request: GitHubRequest): string => {
	const { owner, path, repo } = request;
	const key = `${owner}/${repo}/${path}`;
	return key;
};

export const getGitHubCache = async (
	request: GitHubRequest,
): Promise<GitHubResponse | null> => {
	const cache = await getCache();
	if (cache === null) {
		return null;
	}
	const key = getKey(request);
	const data = (await cache.get<null | GitHubResponse>(key)) ?? null;
	return data;
};

export const setGitHubCache = async (
	request: GitHubRequest,
	response: GitHubResponse,
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getKey(request);
	await cache.set(key, response);
};
