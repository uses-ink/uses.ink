import { RENDER_CACHE_TTL } from "@/lib/constants";
import { getCache } from ".";
import { serverLogger } from "../logger";
import crypto from "node:crypto";
import type { CompileResult } from "../mdx";

export const getCompileCache = async (
	content: string,
): Promise<CompileResult | null> => {
	serverLogger.debug("getCompileCache");
	const cache = await getCache();
	if (cache === null) {
		serverLogger.debug("cache is null");
		return null;
	}
	const key = getCompileKey(content);
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

export const setCompileCache = async (
	content: string,
	compiled: CompileResult,
): Promise<void> => {
	const cache = await getCache();
	if (cache === null) return;
	const key = getCompileKey(content);
	const toSet = Buffer.from(JSON.stringify(compiled)).toString("base64");
	try {
		await cache.set(key, toSet, "EX", RENDER_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setCompileCache" });
	}
};

export const getCompileKey = (content: string): string => {
	const hash = crypto.createHash("sha256");
	hash.update(content);
	return hash.digest("hex");
};
