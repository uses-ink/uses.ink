import { Octokit } from "octokit";

const octokit: { current: Octokit | null } = { current: null };

export const getOctokit = (): Octokit => {
	if (octokit.current === null) {
		octokit.current = new Octokit({
			auth: process.env.GITHUB_TOKEN,
		});
	}
	return octokit.current;
};
