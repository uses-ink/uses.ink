import type { GithubFile, GithubRequest } from "@uses.ink/types";
import { ZodError, type z } from "zod";
import { fetchGitHubContent } from "./github/content";
// import { logger } from "@uses.ink/server-logger";

export const fetchGithubJSON = async (
	request: GithubRequest,
	schema: z.ZodSchema,
) => {
	const content = await fetchGithubRaw(request);
	// logger.debug("content", content);
	try {
		const json = JSON.parse(content);

		return schema.parse(json);
	} catch (error) {
		if (error instanceof ZodError) {
			throw error;
		}
		throw new Error("Invalid JSON");
	}
};

export const fetchGithubRaw = async (request: GithubRequest) => {
	const raw = await fetchGitHubContent(request);

	if (Array.isArray(raw)) throw Error("Post should not be a dir");
	if (typeof raw === "string") throw Error("Response should be in json");
	if (raw.type !== "file") throw Error(`Unknown type "${raw.type}"`);

	const { content } = raw as GithubFile;
	return Buffer.from(content, "base64").toString("utf-8");
};
