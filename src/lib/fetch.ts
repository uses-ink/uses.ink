import { dirname, join } from "node:path";
import { promises as fs } from "node:fs";
import type { DataResponse, GitHubRequest } from "./types";
import { fetchPost } from "./post";
import { fetchReadme } from "./readme";
import { isErrorHasStatus } from "./github";
import { CONFIG_FILE } from "./constants";

export const fetchData = async (
	request: GitHubRequest,
): Promise<DataResponse> => {
	try {
		const { content, lastCommit } = await (["mdx", "md"].includes(
			request.path.split(".").pop() ?? "",
		)
			? fetchPost
			: fetchReadme)(request);

		return { content, lastCommit, error: undefined };
	} catch (error: any) {
		console.error("Error fetching data", error);
		if (isErrorHasStatus(error)) {
			switch (error.status) {
				case 404:
					return { content: null, lastCommit: null, error: "Not found" };
				default:
					return {
						content: null,
						lastCommit: null,
						error: `Error status ${error.status}`,
					};
			}
		}
		return { content: null, lastCommit: null, error: error.toString() };
	}
};

export const fetchConfig = async (request: GitHubRequest) => {
	const configPath = join(dirname(request.path), CONFIG_FILE);
	try {
		const content = await fetchLocalData(configPath);
		return content;
	} catch (error: any) {
		console.error("Error fetching config", error);
		return { content: null, lastCommit: null, error: error.toString() };
	}
};

export const fetchLocalData = async (path: string) => {
	console.log("path", path);
	// Remove leading slash
	const trimmedPath =
		(path.startsWith("/") ? path.slice(1) : path) || "README.md";
	console.log("trimmedPath", trimmedPath);
	const toRead = join(process.cwd(), "blog", trimmedPath);
	console.log("toRead", toRead);
	const content = await fs.readFile(toRead, "utf-8");
	return { content, lastCommit: null, error: undefined };
};
