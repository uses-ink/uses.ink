import { FetchError } from "@uses.ink/errors";
import type { GithubContent, GitHubRequest, GithubTree } from "@uses.ink/types";
import { fetchGithubLastCommit } from "./github/commit";
import { fetchGitHubContent } from "./github/content";

export const fetchReadme = async (
	request: GitHubRequest,
	{ tree }: GithubTree,
) => {
	const raw = await fetchGitHubContent(request);

	// Exceptions
	if (Array.isArray(raw)) throw Error("README should not be a dir");
	if (typeof raw === "string") throw Error("Response should be in json");
	if (raw.type !== "file") throw Error(`Unknown type "${raw.type}"`);

	const { content, name: fileName } = raw as GithubContent;

	const lastCommit = await fetchGithubLastCommit(request);

	if (content) {
		return {
			content: Buffer.from(content, "base64").toString("utf-8"),
			lastCommit,
			fileName,
		};
	}
	throw new FetchError("NOT_FOUND", 404);
};
