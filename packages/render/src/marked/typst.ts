import { NodeCompiler } from "@myriaddreamin/typst-ts-node-compiler";
import { logger } from "@uses.ink/server-logger";
import { getTypstCache, setTypstCache } from "@uses.ink/cache";
import { compilerIns } from "../typst";
import type { MarkedExtension } from "marked";
const inlineRule =
	/^(\${1,2})(?!\$)((?:\\.|[^\\\n])*?(?:\\.|[^\\\n\$]))\1(?=[\s?!\.,:？！。，：]|$)/;
const inlineRuleNonStandard =
	/^(\${1,2})(?!\$)((?:\\.|[^\\\n])*?(?:\\.|[^\\\n\$]))\1/; // Non-standard, even if there are no spaces before and after $ or $$, try to parse

// biome-ignore lint/correctness/noEmptyCharacterClassInRegex: <explanation>
const blockRule = /^(\${1,2})\n((?:\\[^]|[^\\])+?)\n\1(?:\n|$)/;

type Options = {
	nonStandard?: boolean;
};

export default function (options: Options = {}): MarkedExtension {
	return {
		extensions: [
			inlineTypst(options, createRenderer(options, false)),
			blockTypst(options, createRenderer(options, true)),
		],
	};
}

function createRenderer(options, newlineAfter) {
	return async (token) => {
		let result = "";
		const value = token.text;
		const displayMode = token.displayMode;
		const start = performance.now();

		try {
			const cached = await getTypstCache({ code: value, displayMode });
			if (cached) {
				logger.debug(`typst cache hit in ${performance.now() - start}ms`);
				result = cached;
			} else {
				result = await renderToSVGString(value, displayMode);
				logger.debug(`rendered typst in ${performance.now() - start}ms`);

				setTypstCache({ code: value, displayMode }, result as any);
			}
		} catch (error) {}

		return result + (newlineAfter ? "\n" : "");
	};
}

function inlineTypst(options: Options, renderer) {
	const nonStandard = options?.nonStandard;
	const ruleReg = nonStandard ? inlineRuleNonStandard : inlineRule;
	return {
		name: "inlineTypst",
		level: "inline",
		start(src) {
			let index: number;
			let indexSrc = src;

			while (indexSrc) {
				index = indexSrc.indexOf("$");
				if (index === -1) {
					return;
				}
				const f = nonStandard
					? index > -1
					: index === 0 || indexSrc.charAt(index - 1) === " ";
				if (f) {
					const possibleTypst = indexSrc.substring(index);

					if (possibleTypst.match(ruleReg)) {
						return index;
					}
				}

				indexSrc = indexSrc.substring(index + 1).replace(/^\$+/, "");
			}
		},
		tokenizer(src, tokens) {
			const match = src.match(ruleReg);
			if (match) {
				return {
					type: "inlineTypst",
					raw: match[0],
					text: match[2].trim(),
					displayMode: match[1].length === 2,
				};
			}
		},
		renderer,
	};
}

function blockTypst(options, renderer) {
	return {
		name: "blockTypst",
		level: "block",
		tokenizer(src, tokens) {
			const match = src.match(blockRule);
			if (match) {
				return {
					type: "blockTypst",
					raw: match[0],
					text: match[2].trim(),
					displayMode: match[1].length === 2,
				};
			}
		},
		renderer,
	};
}

export async function renderToSVGString(
	code: string,
	displayMode: boolean,
): Promise<any> {
	if (!compilerIns.current) {
		compilerIns.current = NodeCompiler.create();
	}
	const $typst = compilerIns.current;
	const res = renderToSVGString_($typst, code, displayMode);
	$typst.evictCache(10);
	return res;
}

async function renderToSVGString_(
	$typst: NodeCompiler,
	code: string,
	displayMode: boolean,
): Promise<any> {
	const inlineMathTemplate = `
    #set page(height: auto, width: auto, margin: 0pt)

    #let s = state("t", (:))

    #let pin(t) = locate(loc => {
      style(styles => s.update(it => it.insert(t, measure(line(length: loc.position().y + 0.25em), styles).width) + it))
    })

    #show math.equation: it => {
      box(it, inset: (top: 0.5em, bottom: 0.5em))
    }

    $pin("l1")${code}$

    #locate(loc => [
      #metadata(s.final(loc).at("l1")) <label>
    ])
  `;
	const displayMathTemplate = `
    #set page(height: auto, width: auto, margin: 0pt)

    $ ${code} $
  `;
	const mainFileContent = displayMode
		? displayMathTemplate
		: inlineMathTemplate;
	const docRes = $typst.compile({ mainFileContent });
	if (!docRes.result) {
		const taken = docRes.takeDiagnostics();
		if (taken === null) {
			return {};
		}
		const diags = $typst.fetchDiagnostics(taken);
		logger.error({ diags, where: "typst -> renderToSVGString" });
		return {};
	}
	const doc = docRes.result;

	const svg = $typst.svg(doc);
	const res: any = {
		svg,
	};
	if (!displayMode) {
		const query = $typst.query(doc, { selector: "<label>" });
		res.baselinePosition = Number.parseFloat(query[0].value.slice(0, -2));
	}

	return res;
}
