import { D2AttributesSchema } from "@uses.ink/schemas";

const attributeRegex =
	/(?<key>[^\s"'=]+)=(?:(?<noQuoteValue>\w+)|'(?<singleQuoteValue>[^']+)'|"(?<doubleQuoteValue>[^"]+))|(?<truthyKey>\w+)/g;

export function getAttributes(attributesStr: string | null | undefined) {
	return D2AttributesSchema.parse(parseAttributes(attributesStr));
}

export function parseAttributes(attributesStr: string | null | undefined) {
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
