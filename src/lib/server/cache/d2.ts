import { RENDER_CACHE_TTL } from "@/lib/constants";
import { getCache, hashObject } from ".";
import { serverLogger } from "../logger";
import type { D2RenderResult } from "../mdx/d2";
import type { DiagramAttributes } from "../mdx/d2/attributes";
import type { D2Config } from "../mdx/d2/config";
import { pack, unpack } from "msgpackr";

export type D2Content = {
	code: string;
	config: D2Config;
	attributes: DiagramAttributes;
};

export const getD2Cache = async (
	content: D2Content,
): Promise<D2RenderResult | null> => {
	serverLogger.debug("getD2Cache");
	const cache = await getCache();
	if (cache === null) {
		serverLogger.debug("cache is null");
		return null;
	}
	const key = getD2Key(content);
	serverLogger.debug({ key });

	try {
		const data = await cache.getBuffer(key);
		serverLogger.debug(`cache hit for ${key}`);
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
		serverLogger.error({ error, where: "setD2Cache" });
	}
};

export const getD2Key = (content: D2Content): string =>
	`d2/${hashObject(content)}`;
