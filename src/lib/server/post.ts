import type { components } from "@octokit/openapi-types";
import { fetchGitHubContent } from "./github/content";
import type { GitHubRequest } from "../types";
import { fetchGithubLastCommit } from "./github/commit";

export const fetchPost = async (request: GitHubRequest) => {
	const { owner, repo, path } = request;
	const raw = await fetchGitHubContent({
		...request,
		path,
	});

	// Exceptions
	if (Array.isArray(raw)) throw Error("Post should not be a dir");
	if (typeof raw === "string") throw Error("Response should be in json");
	if (raw.type !== "file") throw Error(`Unknown type "${raw.type}"`);

	const contents = raw as components["schemas"]["content-file"];
	const lastCommit = await fetchGithubLastCommit(request);

	if (contents.content) {
		return {
			content: Buffer.from(contents.content, "base64").toString("utf-8"),
			lastCommit,
			fileName: contents.name,
		};
	}
	throw Error(`No post found in ${owner}/${repo}`);
};
