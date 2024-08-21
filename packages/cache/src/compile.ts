import { RENDER_CACHE_TTL } from "@uses.ink/constants";
import { xxh64 } from "@node-rs/xxhash";
import { getCache } from ".";
import { logger } from "@uses.ink/server-logger";
import { pack, unpack } from "msgpackr";
import type { MarkdownCompileResult } from "@uses.ink/types";

export const getCompileCache = async (
	content: string,
): Promise<MarkdownCompileResult | null> => {
	logger.debug("getCompileCache");
	const cache = await getCache();
	if (cache === null) {
		logger.debug("cache is null");
		return null;
	}
	const start = performance.now();
	const key = getCompileKey(content);
	logger.debug(
		`getCompileKey took ${performance.now() - start}ms for ${content.length} bytes`,
	);

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

export const setCompileCache = async (
	content: string,
	compiled: MarkdownCompileResult,
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getCompileKey(content);
	try {
		await cache.set(key, pack(compiled), "EX", RENDER_CACHE_TTL);
	} catch (error) {
		logger.error({ error, where: "setCompileCache" });
	}
};

export const getCompileKey = (content: string): string =>
	`compile/${xxh64(content).toString(36)}`;
