// import { runSync } from "@mdx-js/mdx";
// @ts-ignore
import * as runtime from "react/jsx-runtime";
import { logger } from "@uses.ink/client-logger";

export async function runMDX(code: string) {
	const { run } = await import("@mdx-js/mdx");
	// Need to run sync so the server build also has full html
	const mdx = await run(code, runtime as any);

	const { default: Content, ...rest } = mdx;
	logger.info("runMDX -> Content", Content);
	logger.info("runMDX -> rest", rest);

	return Content;
}
