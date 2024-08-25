import { getGitHubCache, setGitHubCache } from "@uses.ink/cache";
import type {
	GithubRequest,
	GithubCommit,
	ParsedGithubCommit,
} from "@uses.ink/types";
import { getOctokit } from "../octokit";
import { isErrorHasStatus } from "../utils";
import { logger } from "@uses.ink/server-logger/index.js";

export type GithubLastCommit = {
	date: string;
	author: GithubAuthor;
	link: string;
};

export type GithubAuthor = {
	name: string;
	login: string;
	avatar: string;
};

const parseGithubCommit = ({
	commit,
	html_url,
	author,
}: GithubCommit): ParsedGithubCommit => {
	const date = commit.author?.date;
	if (!date) throw new Error("No date found in commit");
	if (!author) throw new Error("No author found in commit");

	return {
		date,
		user: {
			name: author.name ?? author.login,
			login: author.login,
			avatar: author.avatar_url,
		},
		link: html_url,
	};
};

export const fetchGithubLastCommit = async (
	request: GithubRequest,
): Promise<ParsedGithubCommit | undefined> => {
	const { owner, path, repo } = request;
	const cached = await getGitHubCache<GithubCommit[]>(request, "commit");
	if (cached?.data) {
		logger.debug("fetchGithubLastCommit: cache hit");
		return parseGithubCommit(cached.data[0]);
	}
	try {
		const response = await getOctokit().rest.repos.listCommits({
			...{ owner, path, repo },
			mediaType: { format: "json" },
		});
		if (!response.data[0]) return undefined;
		setGitHubCache(request, response, "commit");
		return parseGithubCommit(response.data[0]);
	} catch (error) {
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status === 404) return undefined;
	}
	return undefined;
};
