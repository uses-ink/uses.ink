import { RENDER_CACHE_TTL } from "astro:env/server";
import { getCache, hashObject } from ".";
import { logger } from "@uses.ink/server-logger";
import { pack, unpack } from "msgpackr";

export type TypstContent = {
	code: string;
	displayMode?: boolean;
};

export const getTypstCache = async (
	content: TypstContent,
): Promise<string | null> => {
	logger.debug("getTypstCache");
	const cache = await getCache();
	if (cache === null) {
		logger.debug("cache is null");
		return null;
	}
	const key = getTypstKey(content);
	// logger.debug({ key });

	try {
		const data = await cache.getBuffer(key);
		logger.debug(`cache hit for ${key}`);
		return data ? unpack(data) : null;
	} catch (error) {
		return null;
	}
};

export const setTypstCache = async (
	content: TypstContent,
	rendered: string,
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getTypstKey(content);
	try {
		await cache.set(key, pack(rendered), "EX", RENDER_CACHE_TTL);
	} catch (error) {
		logger.error({ error, where: "setTypstCache" });
	}
};

export const getTypstKey = (content: TypstContent): string =>
	`typst/${hashObject(content)}`;
