import { RENDER_CACHE_TTL } from "@/lib/constants";
import { getCache } from ".";
import { serverLogger } from "../logger";
import crypto from "node:crypto";
import type { D2Config } from "../mdx/d2/config";
import type { DiagramAttributes } from "../mdx/d2/attributes";
import type { D2RenderResult } from "../mdx/d2";

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
		const data = await cache.get(key);
		serverLogger.debug(`cache hit for ${key}`);
		if (!data) return null;
		const parsedData = Buffer.from(data, "base64").toString();
		return JSON.parse(parsedData);
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
	const toSet = Buffer.from(JSON.stringify(rendered)).toString("base64");
	try {
		await cache.set(key, toSet, "EX", RENDER_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setD2Cache" });
	}
};

export const getD2Key = (content: D2Content): string => {
	const hash = crypto.createHash("sha256");
	hash.update(JSON.stringify(content));
	return `d2/${hash.digest("hex")}`;
};
