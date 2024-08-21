import { logger } from "@uses.ink/server-logger";
import { Octokit } from "octokit";

const octokit: { current: Octokit | null; hasWarned: boolean } = {
	current: null,
	hasWarned: false,
};

export const getOctokit = (): Octokit => {
	if (octokit.current === null) {
		const auth = import.meta.env.GITHUB_TOKEN;
		if (!auth) {
			if (!octokit.hasWarned) {
				octokit.hasWarned = true;
				logger.warn("GITHUB_TOKEN is not set");
			}
		}
		octokit.current = new Octokit({
			auth,
		});
	}
	return octokit.current;
};
