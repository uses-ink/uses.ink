import { z } from "zod";

export const D2AttributesSchema = z
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

export const D2ConfigSchema = z
	.object({
		/**
		 * Defines the layout engine to use to generate the diagrams.
		 *
		 * @default 'dagre'
		 * @see https://d2lang.com/tour/layouts#layout-engines
		 */
		layout: z.enum(["dagre", "elk", "tala"]).default("dagre"),
		/**
		 * The padding (in pixels) around the rendered diagrams.
		 *
		 * @default 100
		 */
		pad: z.number().default(100),
		/**
		 * Whether to render the diagrams as if they were sketched by hand.
		 *
		 * @default false
		 */
		sketch: z.boolean().default(false),
	})
	.default({});

export const MathEngineSchema = z.enum(["katex", "typst"]).default("typst");
export const LayoutSchema = z.enum(["post", "gallery"]).default("post");
export const ThemeSchema = z
	.enum(["default", "gruvbox", "solarized", "nord", "monokai"])
	.default("default");
export const FontSchema = z.enum(["sans", "serif", "mono"]).default("sans");

export const RepoConfigSchema = z.object({
	hideTop: z.boolean().optional(),
	readingTime: z.boolean().optional(),
	math: MathEngineSchema.optional(),
	noHighlight: z.boolean().optional(),
	layout: LayoutSchema.optional(),
	nav: z.record(z.string()).optional(),
	defaultCopy: z.boolean().optional(),
	theme: ThemeSchema.optional(),
	font: FontSchema.optional(),
	// renderer: z.enum(["mdx", "marked", "markdown-it"]).optional(),
});

export const MetaSchema = z.object({
	title: z.string().optional(),
	description: z.string().optional(),
	date: z.string().optional(),
	author: z.string().optional(),
	image: z.string().optional(),
	...RepoConfigSchema.shape,
});

export const UserConfigSchema = z.object({
	defaultRepo: z.string().optional(),
	defaultRef: z.string().optional(),
	...RepoConfigSchema.shape,
});
