import { runSync } from "@mdx-js/mdx";
import { MetaSchema } from "../types";
// @ts-ignore
import * as runtime from "react/jsx-runtime";
import type { readingTime as getReadingTime } from "reading-time-estimator";

export function runMDX(code: string) {
	// Need to run sync so the server build also has full html
	const mdx = runSync(code, runtime as any);

	const { default: Content, matter, readingTime } = mdx;
	console.log("runMDX -> readingTime", readingTime);
	console.log("runMDX -> matter", matter);
	const meta = MetaSchema.safeParse(matter ?? {});
	console.log("runMDX -> meta", meta);

	return {
		Content,
		meta,
		readingTime: (readingTime as ReturnType<typeof getReadingTime>) ?? {},
	};
}
