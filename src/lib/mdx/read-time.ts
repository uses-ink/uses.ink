import { readingTime, type SupportedLanguages } from "reading-time-estimator";
import { toString as mdToString } from "mdast-util-to-string";
import type { Plugin } from "unified";
import { valueToEstree } from "estree-util-value-to-estree";

interface RemarkReadingTimeOptions {
	name?: string;
	wordsPerMinute?: number;
	language?: SupportedLanguages;
}

export const remarkReadingTime: Plugin<[RemarkReadingTimeOptions?]> = ({
	language,
	wordsPerMinute,
	name = "readingTime",
}: RemarkReadingTimeOptions = {}) => {
	return (tree, file) => {
		const textOnPage = mdToString(tree);
		const time = readingTime(textOnPage, wordsPerMinute, language);

		// biome-ignore lint/suspicious/noExplicitAny: I know what I'm doing
		(tree as any).children.unshift({
			type: "mdxjsEsm",
			value: "",
			data: {
				estree: {
					type: "Program",
					sourceType: "module",
					body: [
						{
							type: "ExportNamedDeclaration",
							specifiers: [],
							declaration: {
								type: "VariableDeclaration",
								kind: "const",
								declarations: [
									{
										type: "VariableDeclarator",
										id: { type: "Identifier", name },
										init: valueToEstree(time, { preserveReferences: true }),
									},
								],
							},
						},
					],
				},
			},
		});
	};
};
