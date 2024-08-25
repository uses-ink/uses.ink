import { RENDER_CACHE_TTL } from "astro:env/server";

import { getCache, hashObject } from ".";
import { logger } from "@uses.ink/server-logger";

import { pack, unpack } from "msgpackr";
import type {
	D2Config,
	D2DiagramAttributes,
	D2RenderResult,
} from "@uses.ink/types";

export type D2Content = {
	code: string;
	config: D2Config;
	attributes: D2DiagramAttributes;
};

export const getD2Cache = async (
	content: D2Content,
): Promise<D2RenderResult | null> => {
	logger.debug("getD2Cache");
	const cache = await getCache();
	if (cache === null) {
		logger.debug("cache is null");
		return null;
	}
	const key = getD2Key(content);
	// logger.debug({ key });

	try {
		const data = await cache.getBuffer(key);
		logger.debug(`cache hit for ${key}`);
		return data ? unpack(data) : null;
	} catch (error) {
		return null;
	}
};

export const setD2Cache = async (
	content: D2Content,
	rendered: D2RenderResult,
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getD2Key(content);
	try {
		await cache.set(key, pack(rendered), "EX", RENDER_CACHE_TTL);
	} catch (error) {
		logger.error({ error, where: "setD2Cache" });
	}
};

export const getD2Key = (content: D2Content): string =>
	`d2/${hashObject(content)}`;
