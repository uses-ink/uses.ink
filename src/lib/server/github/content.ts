import type { GitHubContent, GitHubRequest } from "@/lib/types";
import { getGitHubCache, setGitHubCache } from "../cache";
import { getOctokit } from "../octokit";
import { isErrorHasStatus } from "../utils";

export const fetchGitHubContent = async (
	request: GitHubRequest,
): Promise<GitHubContent> => {
	const { owner, path, repo } = request;
	const cached = await getGitHubCache(request);
	try {
		const response = await getOctokit().rest.repos.getContent({
			...{ owner, path, repo },
			headers: { "If-None-Match": cached?.headers.etag },
			mediaType: { format: "json" },
		});
		setGitHubCache(request, response);
		return response.data;
	} catch (error) {
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status !== 304) throw error;
		if (cached === null) throw Error("No cache but 304");
		return cached.data;
	}
};
