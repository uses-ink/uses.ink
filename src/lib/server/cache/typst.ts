import { RENDER_CACHE_TTL } from "@/lib/constants";
import { getCache, hashObject } from ".";
import { serverLogger } from "../logger";
import { pack } from "msgpackr";

export type TypstContent = {
	code: string;
	displayMode: boolean;
};

export const getTypstCache = async (
	content: TypstContent,
): Promise<string | null> => {
	serverLogger.debug("getTypstCache");
	const cache = await getCache();
	if (cache === null) {
		serverLogger.debug("cache is null");
		return null;
	}
	const key = getTypstKey(content);
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

export const setTypstCache = async (
	content: TypstContent,
	rendered: string,
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getTypstKey(content);
	const toSet = pack(rendered);
	try {
		await cache.set(key, toSet, "EX", RENDER_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setTypstCache" });
	}
};

export const getTypstKey = (content: TypstContent): string =>
	`typst/${hashObject(content)}`;
