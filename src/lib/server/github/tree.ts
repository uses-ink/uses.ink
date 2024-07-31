import { DEFAULT_BRANCH } from "@/lib/constants";
import type { GitHubRequest } from "@/lib/types";
import type { operations } from "@octokit/openapi-types";
import { getGitHubCache, setGitHubCache } from "../cache";
import { getOctokit } from "../octokit";
import { isErrorHasStatus } from "../utils";

export type GithubTree =
	operations["git/get-tree"]["responses"]["200"]["content"]["application/json"];

export const fetchGithubTree = async (
	request: GitHubRequest,
): Promise<GithubTree> => {
	const { owner, repo, branch } = request;
	const cached = await getGitHubCache<GithubTree>(request, "tree");
	try {
		const response = await getOctokit().rest.git.getTree({
			...{ owner, repo, tree_sha: branch ?? DEFAULT_BRANCH },
			recursive: "true",
			headers: { "If-None-Match": cached?.headers.etag },
			mediaType: { format: "json" },
		});
		setGitHubCache(request, response, "tree");
		return response.data;
	} catch (error) {
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status !== 304) throw error;
		if (cached === null) throw Error("No cache but 304");
		return cached.data;
	}
};
