import { DEFAULT_REF } from "@uses.ink/constants";
import { GITHUB_CACHE_TTL } from "astro:env/server";
import type { GithubContent, GithubRequest } from "@uses.ink/types";
import type { OctokitResponse } from "@octokit/types";
import { type CacheType, getCache } from ".";
import { logger } from "@uses.ink/server-logger";
import { pack, unpack } from "msgpackr";

export const getGitHubCache = async <R = GithubContent>(
	request: GithubRequest,
	type: CacheType = "content",
): Promise<OctokitResponse<R> | null> => {
	// logger.debug("getGitHubCache");
	const cache = await getCache();
	if (cache === null) {
		logger.debug("cache is null");
		return null;
	}
	const key = getGithubKey(request, type);
	// logger.debug({ key });

	try {
		const data = await cache.getBuffer(key);
		return data ? unpack(data) : null;
	} catch (error) {
		return null;
	}
};

export const setGitHubCache = async <R = GithubContent>(
	request: GithubRequest,
	response: OctokitResponse<R> | undefined,
	type: CacheType = "content",
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getGithubKey(request, type);
	try {
		await cache.set(key, pack(response), "EX", GITHUB_CACHE_TTL);
	} catch (error) {
		logger.error({ error, where: "setGitHubCache" });
	}
};

export const getGithubKey = (
	request: GithubRequest,
	type: CacheType,
): string => {
	const { owner, path, repo, ref } = request;
	const key = `${owner}/${repo}/${path}/${ref ?? DEFAULT_REF}/${type}`;
	return key;
};
