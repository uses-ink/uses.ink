import { RENDER_CACHE_TTL } from "astro:env/server";

import { xxh64 } from "@node-rs/xxhash";
import { logger } from "@uses.ink/server-logger";
import type { RepoConfig } from "@uses.ink/types";
import { pack, unpack } from "msgpackr";
import { getCache } from ".";

export type MarkdownEngines = "mdx" | "marked" | "markdownIt";

export type MarkdownResults = {
	mdx: string;
	marked: string;
	markdownIt: string;
};

export type CacheContent = {
	content: string;
	config?: RepoConfig;
};

// Use a mapped type to infer the correct result type based on the engine
export const getCompileCache = async <T extends MarkdownEngines>(
	content: CacheContent,
	type: T,
): Promise<MarkdownResults[T] | null> => {
	logger.debug("getCompileCache");
	const cache = await getCache();
	if (cache === null) {
		logger.debug("cache is null");
		return null;
	}
	const start = performance.now();
	const key = getCompileKey(content, type);
	logger.debug(`getCompileKey took ${performance.now() - start}ms`);

	try {
		const start = performance.now();
		const data = await cache.getBuffer(key);
		logger.debug(
			`cache.getBuffer took ${performance.now() - start}ms for ${key}`,
		);
		return data ? unpack(data) : null;
	} catch (error) {
		return null;
	}
};

export const setCompileCache = async <T extends MarkdownEngines>(
	content: CacheContent,
	compiled: MarkdownResults[T],
	type: T,
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;

	const key = getCompileKey(content, type);

	try {
		await cache.set(key, pack(compiled), "EX", RENDER_CACHE_TTL);
	} catch (error) {
		logger.error({ error, where: "setCompileCache" });
	}
};

export const getCompileKey = (
	content: CacheContent,
	type: MarkdownEngines,
): string => `compile/${xxh64(pack(content)).toString(36)}/${type}`;
