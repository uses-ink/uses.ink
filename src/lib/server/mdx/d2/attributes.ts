import { z } from "zod";

export const AttributesSchema = z
	.object({
		/**
		 * When specified, the diagram will package multiple boards as 1 SVG which transitions through each board at the
		 * specified interval (in milliseconds).
		 */
		animateInterval: z.string().optional(),
		/**
		 * Overrides the global `pad` configuration for the diagram.
		 */
		pad: z.coerce.number().optional(),
		/**
		 * Overrides the global `sketch` configuration for the diagram.
		 */
		sketch: z
			.enum(["true", "false"])
			.optional()
			.transform((value) => {
				if (value === "true") {
					return true;
				}
				if (value === "false") {
					return false;
				}
				return undefined;
			}),
		/**
		 * Defines the target board to render when using composition.
		 * Use `root` to target the root board.
		 *
		 * @see https://d2lang.com/tour/composition
		 */
		target: z
			.string()
			.optional()
			.transform((value) => (value === "root" ? "" : value)),
		/**
		 * The title of the diagram that will be used as the `alt` attribute of the generated image.
		 */
		title: z.string().optional(),
		/**
		 * The width (in pixels) of the diagram.
		 */
		width: z.coerce.number().optional(),

		doNotRender: z
			.enum(["true", "false"])
			.optional()
			.transform((value) => {
				if (value === "true") {
					return true;
				}
				if (value === "false") {
					return false;
				}
				return undefined;
			}),
	})
	.default({});

const attributeRegex =
	/(?<key>[^\s"'=]+)=(?:(?<noQuoteValue>\w+)|'(?<singleQuoteValue>[^']+)'|"(?<doubleQuoteValue>[^"]+))|(?<truthyKey>\w+)/g;

export function getAttributes(attributesStr: string | null | undefined) {
	return AttributesSchema.parse(parseAttributes(attributesStr));
}

function parseAttributes(attributesStr: string | null | undefined) {
	if (!attributesStr) {
		return {};
	}

	const matches = attributesStr.matchAll(attributeRegex);

	const attributes: Record<string, string> = {};

	for (const match of matches) {
		const { key, noQuoteValue, singleQuoteValue, doubleQuoteValue, truthyKey } =
			match.groups ?? {};

		const attributeKey = truthyKey ?? key;
		const attributeValue = truthyKey
			? "true"
			: noQuoteValue ?? singleQuoteValue ?? doubleQuoteValue;

		if (attributeKey && attributeValue) {
			attributes[attributeKey] = attributeValue;
		}
	}

	return attributes;
}

export type DiagramAttributes = z.infer<typeof AttributesSchema>;
