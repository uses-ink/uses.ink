import { GITHUB_CACHE_TTL } from "@/lib/constants";
import type { GitHubContent, GitHubRequest } from "@/lib/types";
import type { OctokitResponse } from "@octokit/types";
import { type CacheType, getCache } from ".";
import { serverLogger } from "../logger";

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
	const key = getGithubKey(request, type);
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
	const key = getGithubKey(request, type);
	const toSet = Buffer.from(JSON.stringify(response)).toString("base64");
	try {
		await cache.set(key, toSet, "EX", GITHUB_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setGitHubCache" });
	}
};

export const getGithubKey = (
	request: GitHubRequest,
	type: CacheType,
): string => {
	const { owner, path, repo, ref } = request;
	const key = `${owner}/${repo}/${path}/${ref}@${type}`;
	return key;
};
