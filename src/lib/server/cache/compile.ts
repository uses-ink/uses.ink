import { RENDER_CACHE_TTL } from "@/lib/constants";
import { xxh64 } from "@node-rs/xxhash";
import { getCache } from ".";
import { serverLogger } from "../logger";
import type { CompileResult } from "../mdx";
import { pack, unpack } from "msgpackr";

export const getCompileCache = async (
	content: string,
): Promise<CompileResult | null> => {
	serverLogger.debug("getCompileCache");
	const cache = await getCache();
	if (cache === null) {
		serverLogger.debug("cache is null");
		return null;
	}
	const start = performance.now();
	const key = getCompileKey(content);
	serverLogger.debug(
		`getCompileKey took ${performance.now() - start}ms for ${content.length} bytes`,
	);

	try {
		const start = performance.now();
		const data = await cache.getBuffer(key);
		serverLogger.debug(
			`cache.getBuffer took ${performance.now() - start}ms for ${key}`,
		);
		return data ? unpack(data) : null;
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
	try {
		await cache.set(key, pack(compiled), "EX", RENDER_CACHE_TTL);
	} catch (error) {
		serverLogger.error({ error, where: "setCompileCache" });
	}
};

export const getCompileKey = (content: string): string =>
	`compile/${xxh64(content).toString(36)}`;
