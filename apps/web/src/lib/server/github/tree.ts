import type { GitHubRequest, GithubTree } from "@uses.ink/types";
import { getGitHubCache, setGitHubCache } from "@uses.ink/cache";
import { getOctokit } from "../octokit";
import { isErrorHasStatus } from "../utils";
import { DEFAULT_REF } from "@uses.ink/constants";
import { FetchError } from "@uses.ink/errors";

export const fetchGithubTree = async (
	request: GitHubRequest,
): Promise<GithubTree> => {
	const { owner, repo, ref } = request;
	const cached = await getGitHubCache<GithubTree>(request, "tree");
	try {
		const response = await getOctokit().rest.git.getTree({
			...{ owner, repo, tree_sha: ref ?? DEFAULT_REF },
			recursive: "true",
			headers: { "If-None-Match": cached?.headers.etag },
			mediaType: { format: "json" },
		});
		setGitHubCache(request, response, "tree");
		return response.data;
	} catch (error) {
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status === 404) throw new FetchError("NOT_FOUND", 404);
		if (error.status !== 304) throw error;
		if (cached === null) throw Error("No cache but 304");
		return cached.data;
	}
};
