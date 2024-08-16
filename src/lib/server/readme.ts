import type { components } from "@octokit/openapi-types";
import { basename, dirname, join } from "node:path";
import { README_FILES } from "../constants";
import { FetchError } from "../errors";
import type { GitHubRequest } from "../types";
import { fetchGithubLastCommit } from "./github/commit";
import { fetchGitHubContent } from "./github/content";
import type { GithubTree } from "./github/tree";

export const fetchReadme = async (request: GitHubRequest, tree: GithubTree) => {
	const path = request.path.trim() || ".";
	const filteredTree = tree.tree.filter((file) => {
		const isFile = file.type === "blob";
		const isMarkdown = file.path?.endsWith(".md");

		const isCurrentDir = dirname(file.path ?? ".") === path;
		return isFile && isMarkdown && isCurrentDir;
	});
	const readme = filteredTree.find((file) =>
		README_FILES.some(
			(f) => basename(file.path ?? "").toLowerCase() === f.toLowerCase(),
		),
	);

	if (!readme) {
		throw new FetchError("NOT_FOUND", 404);
	}

	const raw = await fetchGitHubContent({
		...request,
		path: readme.path ?? join(path, "README.md"),
	});

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
