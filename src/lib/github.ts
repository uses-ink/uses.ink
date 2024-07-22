import type { GitHubContent, GitHubRequest, GithubCommit } from "./types";

import { getGitHubCache, setGitHubCache } from "./cache";
import { getOctokit } from "./octokit";

export const fetchGitHubContent = async (
	request: GitHubRequest,
): Promise<GitHubContent> => {
	const { owner, path, repo } = request;
	const cached = await getGitHubCache(request);
	try {
		const response = await getOctokit().rest.repos.getContent({
			...{ owner, path, repo },
			headers: { "If-None-Match": cached?.headers.etag },
			mediaType: { format: "json" },
		});
		setGitHubCache(request, response);
		return response.data;
	} catch (error) {
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status !== 304) throw error;
		if (cached === null) throw Error("No cache but 304");
		return cached.data;
	}
};

export const fetchGithubLastCommit = async (
	request: GitHubRequest,
): Promise<{
	date: string;
	author: {
		name: string;
		login: string;
		avatar: string;
	};
	link: string;
} | null> => {
	const { owner, path, repo } = request;
	const cached = await getGitHubCache<GithubCommit>(request, "commit");
	try {
		const response = await getOctokit().rest.repos.listCommits({
			...{ owner, path, repo },
			headers: { "If-None-Match": cached?.headers.etag },
			mediaType: { format: "json" },
		});
		setGitHubCache(request, response, "commit");
		console.log("res", response.data[0]);
		const date = response.data[0].commit.author?.date;
		const author = {
			name: response.data[0].commit.author?.name,
			login: response.data[0].author?.login,
			avatar: response.data[0].author?.avatar_url,
		};
		const link = response.data[0].html_url;
		if (!date || !author.name || !author.login || !author.avatar || !link)
			return null;
		return { date, author: author as any, link };
	} catch (error) {
		// Return cache
		if (!isErrorHasStatus(error)) throw error;
		if (error.status !== 304) throw error;
		if (cached === null) throw Error("No cache but 304");
		const date = cached.data[0].commit.author?.date;
		const author = {
			name: cached.data[0].commit.author?.name,
			login: cached.data[0].author?.login,
			avatar: cached.data[0].author?.avatar_url,
		};
		const link = cached.data[0].html_url;
		if (!date || !author.name || !author.login || !author.avatar || !link)
			return null;
		return { date, author: author as any, link };
	}
};

export const isErrorHasStatus = (
	raw: unknown,
): raw is {
	status: number;
} => {
	if (typeof raw !== "object") return false;
	if (raw === null) throw raw;
	if ("status" in raw) {
		return typeof (raw as { status: number }).status === "number";
	}
	return false;
};
