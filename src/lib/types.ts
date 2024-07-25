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
			fileName: string;
			error: undefined;
	  }
	| {
			content: null;
			lastCommit: null;
			fileName: null;
			error: string;
	  };

export type CommitResponse = {
	date: string;
	author: AuthorResponse | null;
	link: string | null;
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
	image: z.string().optional(),
	nav: z.record(z.string()).optional(),
});

export const ConfigSchema = z.object({
	hideTop: z.boolean().optional(),
	readingTime: z.boolean().optional(),
});

// Utility type to get the type of a promise
export type PromiseOf<T> = T extends Promise<infer U> ? U : T;
