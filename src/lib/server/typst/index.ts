import { NodeCompiler } from "@myriaddreamin/typst-ts-node-compiler";
import { serverLogger } from "../logger";
import { getTypstCache, setTypstCache } from "../cache";
import type { z } from "zod";
import type { ConfigSchema } from "@/lib/types";

const compilerIns: { current: NodeCompiler | null } = { current: null };

export async function renderToSVGString(code: string): Promise<any> {
	if (!compilerIns.current) {
		compilerIns.current = NodeCompiler.create();
	}
	const $typst = compilerIns.current;
	const res = renderToSVGString_($typst, code);
	$typst.evictCache(10);
	return res;
}

export const compileTypst = async (
	content: string,
	config?: z.infer<typeof ConfigSchema>,
) => {
	const cached = await getTypstCache({ code: content });
	if (cached) return cached;
	const res = await renderToSVGString(content);
	await setTypstCache({ code: content }, res);
	return res;
};

async function renderToSVGString_(
	$typst: NodeCompiler,
	code: string,
): Promise<string | null> {
	const mainFileContent = `#set text(size: 24pt)
#set page(margin: (top: 2pt, bottom: 0pt, left: 0pt, right: 0pt))
${code}`;
	const docRes = $typst.compile({ mainFileContent });
	if (!docRes.result) {
		const taken = docRes.takeDiagnostics();
		if (taken === null) {
			return null;
		}
		const diags = $typst.fetchDiagnostics(taken);
		serverLogger.error({ diags, where: "typst -> renderToSVGString" });
		return null;
	}
	const doc = docRes.result;

	const svg = $typst.svg(doc);
	return svg;
}
