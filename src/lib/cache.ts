import type { OctokitResponse } from "@octokit/types";
import { CACHE_TTL } from "./constants";
import type { GitHubContent, GitHubRequest } from "./types";
import { Redis } from "ioredis";

const cache: { current: Redis | null } = {
	current: null,
};

const makeCache = async () => {
	console.log("makeCache");
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
	console.log("getGitHubCache");
	const cache = await getCache();
	console.log("cache", cache);
	if (cache === null) {
		console.log("cache is null");
		return null;
	}
	const key = getKey(request, type);
	console.log("key", key);

	// const data = (await cache.get<null | OctokitResponse<R>>(key)) ?? null;
	const data = await cache.get(key);
	const parsedData = data
		? JSON.parse(Buffer.from(data, "base64").toString())
		: null;

	console.log("data", parsedData);
	return parsedData;
};

export const setGitHubCache = async <R = GitHubContent>(
	request: GitHubRequest,
	response: OctokitResponse<R>,
	type: CacheType = "content",
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getKey(request, type);
	const toSet = Buffer.from(JSON.stringify(response)).toString("base64");
	await cache.set(key, toSet, "EX", CACHE_TTL);
};
