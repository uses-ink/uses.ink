import type { components } from "@octokit/openapi-types";
import { basename, dirname, join } from "node:path";
import { README_FILES } from "../constants";
import { FetchError } from "../errors";
import type { GitHubRequest } from "../types";
import { fetchGithubLastCommit } from "./github/commit";
import { fetchGitHubContent } from "./github/content";
import type { GithubTree } from "./github/tree";
import { filterTree } from "./utils";

export const fetchReadme = async (
	request: GitHubRequest,
	{ tree }: GithubTree,
) => {
	const raw = await fetchGitHubContent(request);

	// Exceptions
	if (Array.isArray(raw)) throw Error("README should not be a dir");
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
	throw new FetchError("NOT_FOUND", 404);
};
