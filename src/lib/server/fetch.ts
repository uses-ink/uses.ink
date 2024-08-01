import { promises as fs } from "node:fs";
import { dirname, join } from "node:path";
import type { components } from "@octokit/openapi-types";
import { CONFIG_FILE, EXTENSIONS } from "../constants";
import { fetchGitHubContent } from "./github/content";
import { fetchPost } from "./post";
import { fetchReadme } from "./readme";
import { ConfigSchema, type DataResponse, type GitHubRequest } from "../types";
import { isErrorHasStatus } from "./utils";
import { FetchError } from "../errors";
import { serverLogger } from "./logger";

export const fetchData = async (
	request: GitHubRequest,
): Promise<DataResponse> => {
	try {
		const { content, lastCommit, fileName } = await ([
			...EXTENSIONS,
			"json",
		].includes(request.path.split(".").pop() ?? "")
			? fetchPost
			: fetchReadme)(request);

		return { content, lastCommit, fileName };
	} catch (error: any) {
		serverLogger.error({ error, where: "fetchData" });
		if (isErrorHasStatus(error)) {
			switch (error.status) {
				case 404:
					throw new FetchError("NOT_FOUND", 404);
				default:
					throw new FetchError("ERROR_STATUS", error.status);
			}
		}
		throw new FetchError("UNKNOWN");
	}
};

export const fetchConfig = async (request: GitHubRequest) => {
	const configPath = join(dirname(request.path), CONFIG_FILE);
	serverLogger.debug({ configPath });
	try {
		const raw = await fetchGitHubContent({ ...request, path: configPath });
		if (Array.isArray(raw)) throw Error("Post should not be a dir");
		if (typeof raw === "string") throw Error("Response should be in json");
		if (raw.type !== "file") throw Error(`Unknown type "${raw.type}"`);

		const { content } = raw as components["schemas"]["content-file"];
		const parsed = Buffer.from(content, "base64").toString("utf-8");
		serverLogger.debug({ parsed });
		try {
			const config = ConfigSchema.parse(JSON.parse(parsed));
			return config;
		} catch (error) {
			return null;
		}
	} catch (error: any) {
		if (isErrorHasStatus(error)) {
			switch (error.status) {
				case 404:
					return null;
				default:
					throw error;
			}
		}
		throw error;
	}
};

export const fetchLocalData = async (path: string): Promise<DataResponse> => {
	try {
		serverLogger.debug({ path });
		// Remove leading slash
		const trimmedPath =
			(path.startsWith("/") ? path.slice(1) : path) || "README.md";
		serverLogger.debug({ trimmedPath });
		const toRead = join(process.cwd(), "docs", trimmedPath);
		serverLogger.debug({ toRead });
		const content = await fs.readFile(toRead, "utf-8");
		const lastModified = (await fs.stat(toRead)).mtime;
		return {
			content,
			lastCommit: {
				author: null,
				date: lastModified.toISOString(),
				link: null,
			},
			fileName: trimmedPath,
		};
	} catch (error: any) {
		const is404 = error.code === "ENOENT";
		if (is404) {
			throw new FetchError("NOT_FOUND", 404);
		}
		throw new FetchError("UNKNOWN");
	}
};
