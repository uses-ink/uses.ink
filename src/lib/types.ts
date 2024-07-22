import type { components, operations } from "@octokit/openapi-types";
import { z } from "zod";

export interface GitHubRequest {
	/** E.g. "thien-do" */
	owner: string;
	/** E.g. "notes" */
	repo: string;
	/** E.g. "dir/foo", "dir/foo/hello.md", "" when at repo root */
	path: string;
}

export type GitHubContent =
	operations["repos/get-content"]["responses"]["200"]["content"]["application/json"];

export type GithubCommit =
	operations["repos/list-commits"]["responses"]["200"]["content"]["application/json"];

export type GitHubFile = components["schemas"]["content-file"];

export type GitHubDir = components["schemas"]["content-directory"];

export type GitHubEntry = GitHubDir[number];

export type DataResponse =
	| {
			content: string;
			lastCommit: CommitResponse | null;
			error: undefined;
	  }
	| {
			content: null;
			lastCommit: null;
			error: string;
	  };

export type CommitResponse = {
	date: string;
	author: AuthorResponse;
	link: string;
};

export type AuthorResponse = {
	name: string;
	login: string;
	avatar: string;
};

export const MetaSchema = z.object({
	title: z.string().optional(),
	description: z.string().optional(),
	date: z.string().optional(),
	author: z.string().optional(),
	hideTop: z.boolean().default(false),
	readingTime: z.boolean().default(true),
});
