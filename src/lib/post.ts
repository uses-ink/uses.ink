import { compile, runSync } from "@mdx-js/mdx";
import remarkGfm from "remark-gfm";
import * as runtime from "react/jsx-runtime";
import { fetchGitHubContent, fetchGithubLastCommit } from "./github";
import type { GitHubRequest } from "./types";
import type { components } from "@octokit/openapi-types";

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
		};
	}
	throw Error(`No post found in ${owner}/${repo}`);
};
