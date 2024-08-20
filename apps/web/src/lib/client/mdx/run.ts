import { runSync } from "@mdx-js/mdx";
// @ts-ignore
import * as runtime from "react/jsx-runtime";
import type { readingTime as getReadingTime } from "reading-time-estimator";
import { clientLogger } from "@uses.ink/logger";

export function runMDX(code: string) {
	// Need to run sync so the server build also has full html
	const mdx = runSync(code, runtime as any);

	const { default: Content, readingTime } = mdx;
	clientLogger.debug("runMDX -> readingTime", readingTime);

	return {
		Content,

		readingTime: (readingTime as ReturnType<typeof getReadingTime>) ?? {},
	};
}
