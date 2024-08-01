import type { z } from "zod";
import { capitalizeFileName } from "../client/utils";
import { README_FILES } from "../constants";
import type { CommitResponse, MetaSchema } from "../types";
import type { RepoRequest } from "./repo-request";

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

export type ResolvedMetadata = {
	title: string;
	description?: string;
	author: { name?: string; link?: string; avatar?: string };
	date?: string;
};

export const resolveMetadata = (
	meta: z.infer<typeof MetaSchema>,
	// Content: React.FC,
	request: RepoRequest,
	lastCommit?: CommitResponse,
	filename?: string,
): ResolvedMetadata => {
	const date = meta.date ?? lastCommit?.date;

	const author = lastCommit?.author
		? {
				name: lastCommit.author.name,
				link: `https://github.com/${lastCommit.author.login}`,
				avatar: lastCommit.author.avatar,
			}
		: { name: meta.author };

	const title =
		meta.title ??
		// resolveTitle(Content) ??
		(filename
			? README_FILES.map((e) => e.toLowerCase()).includes(
					filename.toLowerCase(),
				)
				? `${request.owner}/${request.repo}`
				: capitalizeFileName(filename)
			: `${request.owner}/${request.repo}`);

	const description = meta.description;

	return {
		title,
		description,
		author,
		date,
	};
};
