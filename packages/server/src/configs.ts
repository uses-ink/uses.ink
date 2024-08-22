import type { GithubRequest, UserConfig } from "@uses.ink/types";
import { fetchGithubJSON } from "./fetch";
import { RepoConfigSchema, UserConfigSchema } from "@uses.ink/schemas";
import { CONFIG_FILE } from "@uses.ink/constants";
import { join, dirname } from "node:path";
// import { logger } from "@uses.ink/server-logger";

export const fetchUserConfig = async (
	request: GithubRequest,
): Promise<UserConfig> =>
	await fetchGithubJSON(
		{ ...request, repo: request.owner, path: CONFIG_FILE },
		UserConfigSchema,
	);
export const fetchRepoConfig = async (
	request: GithubRequest,
): Promise<UserConfig> =>
	await fetchGithubJSON(
		{ ...request, path: join(dirname(request.path), CONFIG_FILE) },
		RepoConfigSchema,
	);
