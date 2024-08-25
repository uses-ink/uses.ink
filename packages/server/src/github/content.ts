import type { GithubContent, GithubRequest } from "@uses.ink/types";
import { getGitHubCache, setGitHubCache } from "@uses.ink/cache";
import { getOctokit } from "../octokit";
import { isErrorHasStatus } from "../utils";
import { DEFAULT_REF } from "@uses.ink/constants";
import { FetchError } from "@uses.ink/errors";
import { logger } from "@uses.ink/server-logger";

export const fetchGitHubContent = async (
	request: GithubRequest,
): Promise<GithubContent> => {
	const { owner, path, repo, ref } = request;
	const cached = await getGitHubCache(request, "content");
	if (cached !== null) {
		if (cached?.data === undefined)
			throw new FetchError(
				"NOT_FOUND",
				`${owner}/${repo}${ref ? `@${ref}` : ""}/${path} (cached)`,
			);
		logger.debug("fetchGitHubContent: cache hit");
		return cached.data;
	}
	const start = performance.now();
	try {
		const response = await getOctokit().rest.repos.getContent({
			...{ owner, path, repo, ref: ref ?? DEFAULT_REF },
			mediaType: { format: "json" },
		});

		setGitHubCache(request, response, "content");
		return response.data;
	} catch (error) {
		setGitHubCache(request, undefined, "content");
		if (!isErrorHasStatus(error)) throw error;
		if (error.status === 404)
			throw new FetchError(
				"NOT_FOUND",
				`${owner}/${repo}${ref ? `@${ref}` : ""}/${path}`,
			);
	} finally {
		const end = performance.now();
		logger.debug(`fetchGitHubContent ${end - start}ms`);
	}
	throw new Error("Unknown error");
};
