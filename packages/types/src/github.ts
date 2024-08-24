import type { components, operations } from "@octokit/openapi-types";

export type GithubRequest = {
	/** E.g. "cestef" */
	owner: string;
	/** E.g. "blog" */
	repo: string;
	/** E.g. "dir/foo", "dir/foo/hello.md", "" when at repo root */
	path: string;
	/** E.g. "main", "feature/branch" */
	ref?: string;
};

export type GithubTree = components["schemas"]["git-tree"];
export type GithubContent =
	operations["repos/get-content"]["responses"]["200"]["content"]["application/json"];

export type GithubCommit = components["schemas"]["commit"];

export type ParsedGithubCommit = {
	date: string;
	user?: ParsedGithubUser;
	link?: string;
};

export type ParsedGithubUser = {
	name: string;
	login: string;
	avatar: string;
};

export type GithubFile = components["schemas"]["content-file"];

export type GithubDir = components["schemas"]["content-directory"];

export type GithubEntry = GithubDir[number];
