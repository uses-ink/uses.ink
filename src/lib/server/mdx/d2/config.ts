import { z } from "zod";

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

export type D2UserConfig = z.input<typeof D2ConfigSchema>;
export type D2Config = z.output<typeof D2ConfigSchema>;
