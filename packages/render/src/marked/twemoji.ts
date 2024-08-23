import twemoji from "twemoji";
import { emojify } from "node-emoji";
import type { MarkedExtension } from "marked";

const twemojiParse = (content) =>
	twemoji.parse(content, {
		ext: ".svg",
		size: "svg",
		base: "https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/",
	});

// Configure marked extension
const markedTwemoji: MarkedExtension = {
	extensions: [
		{
			name: "twemoji",
			level: "inline",
			start(src) {
				return src.indexOf(":");
			},
			tokenizer(src, _) {
				const rule = /^:(\w+):/;
				const match = rule.exec(src);
				if (match) {
					return {
						type: "twemoji",
						raw: match[0],
						emoji: match[1],
					};
				}
			},
			renderer(token) {
				return twemojiParse(emojify(token.raw));
			},
		},
	],
};

export default markedTwemoji;
