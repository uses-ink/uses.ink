import path from "node:path";
import type { components } from "@octokit/openapi-types";
import { README_FILES } from "../constants";
import { fetchGitHubContent } from "./github/content";
import type { GitHubRequest } from "../types";
import { fetchGithubLastCommit } from "./github/commit";
import { serverLogger } from "./logger";

export const fetchReadme = async (request: GitHubRequest) => {
	const { owner, repo } = request;
	for (const file of README_FILES) {
		serverLogger.debug({ file });
		try {
			const raw = await fetchGitHubContent({
				...request,
				path: path.join(request.path, file),
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
		} catch (error) {
			serverLogger.debug({ error, where: "fetchReadme" });
		}
	}
	throw Error(`No README found in ${owner}/${repo}`);
};
