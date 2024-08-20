import { DEFAULT_REF, GITHUB_CACHE_TTL } from "@uses.ink/constants";
import type { GitHubContent, GitHubRequest } from "@uses.ink/types";
import type { OctokitResponse } from "@octokit/types";
import { type CacheType, getCache } from "..";
import { serverLogger } from "@uses.ink/logger";
import { pack, unpack } from "msgpackr";

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
		const data = await cache.getBuffer(key);
		return data ? unpack(data) : null;
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
	try {
		await cache.set(key, pack(response), "EX", GITHUB_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setGitHubCache" });
	}
};

export const getGithubKey = (
	request: GitHubRequest,
	type: CacheType,
): string => {
	const { owner, path, repo, ref } = request;
	const key = `${owner}/${repo}/${path}/${ref ?? DEFAULT_REF}/${type}`;
	return key;
};
