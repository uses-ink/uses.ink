import type { GithubContent, GithubRequest } from "@uses.ink/types";
import { getGitHubCache, setGitHubCache } from "@uses.ink/cache";
import { getOctokit } from "../octokit";
import { isErrorHasStatus } from "../utils";
import { DEFAULT_REF } from "@uses.ink/constants";
// import { logger } from "@uses.ink/server-logger/index.js";

export const fetchGitHubContent = async (
	request: GithubRequest,
): Promise<GithubContent> => {
	const { owner, path, repo, ref } = request;
	const cached = await getGitHubCache(request, "content");
	// logger.debug("cached", cached);
	try {
		const response = await getOctokit().rest.repos.getContent({
			...{ owner, path, repo, ref: ref ?? DEFAULT_REF },
			headers: { "If-None-Match": cached?.headers.etag },
			mediaType: { format: "json" },
		});
		setGitHubCache(request, response, "content");
		return response.data;
	} catch (error) {
		// logger.error({ error, where: "fetchGitHubContent" });
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status === 404)
			throw Error(`Not found: ${owner}/${repo}${ref ? `@${ref}` : ""}/${path}`);
		if (error.status !== 304) throw error;
		if (cached === null) throw Error("No cache but 304");
		return cached.data;
	}
};
