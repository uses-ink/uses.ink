import { logger } from "@uses.ink/server-logger";
import { Octokit } from "octokit";
import { GITHUB_TOKEN } from "astro:env/server";

const octokit: { current: Octokit | null; hasWarned: boolean } = {
	current: null,
	hasWarned: false,
};

export const getOctokit = (): Octokit => {
	if (octokit.current === null) {
		if (!GITHUB_TOKEN) {
			if (!octokit.hasWarned) {
				octokit.hasWarned = true;
				logger.warn("GITHUB_TOKEN is not set");
			}
		}
		octokit.current = new Octokit({
			auth: GITHUB_TOKEN,
		});
	}
	return octokit.current;
};
