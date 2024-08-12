import { RENDER_CACHE_TTL } from "@/lib/constants";
import { getCache } from ".";
import { serverLogger } from "../logger";
import crypto from "node:crypto";

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
	const toSet = Buffer.from(JSON.stringify(rendered)).toString("base64");
	try {
		await cache.set(key, toSet, "EX", RENDER_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setTypstCache" });
	}
};

export const getTypstKey = (content: TypstContent): string => {
	const hash = crypto.createHash("sha256");
	hash.update(content.code);
	return `typst/${hash.digest("hex")}/${content.displayMode}`;
};
