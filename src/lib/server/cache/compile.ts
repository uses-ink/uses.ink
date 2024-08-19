import { RENDER_CACHE_TTL } from "@/lib/constants";
import { xxh64 } from "@node-rs/xxhash";
import { getCache } from ".";
import { serverLogger } from "../logger";
import type { CompileResult } from "../mdx";
import { pack } from "msgpackr";

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
	const toSet = pack(compiled);
	try {
		await cache.set(key, toSet, "EX", RENDER_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setCompileCache" });
	}
};

export const getCompileKey = (content: string): string =>
	`compile/${xxh64(content).toString(36)}`;
