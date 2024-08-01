import type { OctokitResponse } from "@octokit/types";
import { CACHE_TTL } from "../constants";
import type { GitHubContent, GitHubRequest } from "../types";
import { Redis } from "ioredis";
import { serverLogger } from "./logger";

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

const getKey = (request: GitHubRequest, type: CacheType): string => {
	const { owner, path, repo } = request;
	const key = `${owner}/${repo}/${path}@${type}`;
	return key;
};

type CacheType = "content" | "commit" | "tree";

export const getGitHubCache = async <R = GitHubContent>(
	request: GitHubRequest,
	type: CacheType = "content",
): Promise<OctokitResponse<R> | null> => {
	serverLogger.debug("getGitHubCache");
	const cache = await getCache();
	if (cache === null) {
		serverLogger.debug("cache is null");
		return null;
	}
	const key = getKey(request, type);
	serverLogger.debug({ key });

	try {
		const data = await cache.get(key);
		const parsedData = data
			? JSON.parse(Buffer.from(data, "base64").toString())
			: null;
		serverLogger.debug(`cache hit for ${key}`);
		return parsedData;
	} catch (error) {
		return null;
	}
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
	try {
		await cache.set(key, toSet, "EX", CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setGitHubCache" });
	}
};
