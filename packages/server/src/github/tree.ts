import type { GithubRequest, GithubTree } from "@uses.ink/types";
import { getGitHubCache, setGitHubCache } from "@uses.ink/cache";
import { getOctokit } from "../octokit";
import { isErrorHasStatus } from "../utils";
import { DEFAULT_REF } from "@uses.ink/constants";
import { FetchError } from "@uses.ink/errors";

export const fetchGithubTree = async (
	request: GithubRequest,
): Promise<GithubTree> => {
	const { owner, repo, ref } = request;
	const cached = await getGitHubCache<GithubTree>(request, "tree");
	if (cached) {
		return cached.data;
	}
	try {
		const response = await getOctokit().rest.git.getTree({
			...{ owner, repo, tree_sha: ref ?? DEFAULT_REF },
			recursive: "true",
			mediaType: { format: "json" },
		});
		setGitHubCache(request, response, "tree");
		return response.data;
	} catch (error) {
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status === 404)
			throw new FetchError(
				"NOT_FOUND",
				`${owner}/${repo}${ref ? `@${ref}` : ""}`,
			);
	}
	throw new Error("Unknown error");
};
