import type { Config } from "tailwindcss";
import tailwindcssAnimate from "tailwindcss-animate";
import tailwindTypography from "@tailwindcss/typography";

const hexToRgb = (h: string) => {
	let hex = h;
	hex = hex.replace("#", "");
	hex = hex.length === 3 ? hex.replace(/./g, "$&$&") : hex;
	const r = Number.parseInt(hex.substring(0, 2), 16);
	const g = Number.parseInt(hex.substring(2, 4), 16);
	const b = Number.parseInt(hex.substring(4, 6), 16);
	return `${r} ${g} ${b}`;
};

export default {
	darkMode: ["class"],
	content: [
		"./src/**/*.{astro,jsx,ts,tsx,html}",
		"../../packages/components/src/**/*.{astro,jsx,ts,tsx,html}",
	],
	theme: {
		container: {
			center: true,
			padding: "2rem",
			screens: {
				"2xl": "1400px",
			},
		},
		extend: {
			colors: {
				border: "hsl(var(--border))",
				input: "hsl(var(--input))",
				ring: "hsl(var(--ring))",
				background: "hsl(var(--background))",
				foreground: "hsl(var(--foreground))",
				primary: {
					DEFAULT: "hsl(var(--primary))",
					foreground: "hsl(var(--primary-foreground))",
				},
				secondary: {
					DEFAULT: "hsl(var(--secondary))",
					foreground: "hsl(var(--secondary-foreground))",
				},
				destructive: {
					DEFAULT: "hsl(var(--destructive))",
					foreground: "hsl(var(--destructive-foreground))",
				},
				muted: {
					DEFAULT: "hsl(var(--muted))",
					foreground: "hsl(var(--muted-foreground))",
				},
				accent: {
					DEFAULT: "hsl(var(--accent))",
					foreground: "hsl(var(--accent-foreground))",
				},
				popover: {
					DEFAULT: "hsl(var(--popover))",
					foreground: "hsl(var(--popover-foreground))",
				},
				card: {
					DEFAULT: "hsl(var(--card))",
					foreground: "hsl(var(--card-foreground))",
				},
			},
			borderRadius: {
				lg: "var(--radius)",
				md: "calc(var(--radius) - 2px)",
				sm: "calc(var(--radius) - 4px)",
			},
			keyframes: {
				"accordion-down": {
					from: { height: "0" },
					to: { height: "var(--radix-accordion-content-height)" },
				},
				"accordion-up": {
					from: { height: "var(--radix-accordion-content-height)" },
					to: { height: "0" },
				},
			},
			animation: {
				"accordion-down": "accordion-down 0.2s ease-out",
				"accordion-up": "accordion-up 0.2s ease-out",
			},
			typography: () => ({
				gruvbox: {
					css: {
						// Light theme
						"--tw-prose-body": "#3c3836",
						"--tw-prose-headings": "#282828",
						"--tw-prose-lead": "#504945",
						"--tw-prose-links": "#076678",
						"--tw-prose-bold": "#282828",
						"--tw-prose-counters": "#7c6f64",
						"--tw-prose-bullets": "#bdae93",
						"--tw-prose-hr": "#d5c4a1",
						"--tw-prose-quotes": "#282828",
						"--tw-prose-quote-borders": "#d5c4a1",
						"--tw-prose-captions": "#7c6f64",
						"--tw-prose-kbd": "#282828",
						"--tw-prose-kbd-shadows": hexToRgb("#282828"),
						"--tw-prose-code": "#9d0006",
						"--tw-prose-pre-code": "#fbf1c7",
						"--tw-prose-pre-bg": "#3c3836",
						"--tw-prose-th-borders": "#bdae93",
						"--tw-prose-td-borders": "#d5c4a1",

						// Dark theme
						"--tw-prose-invert-body": "#ebdbb2",
						"--tw-prose-invert-headings": "#fbf1c7",
						"--tw-prose-invert-lead": "#d5c4a1",
						"--tw-prose-invert-links": "#83a598",
						"--tw-prose-invert-bold": "#fbf1c7",
						"--tw-prose-invert-counters": "#bdae93",
						"--tw-prose-invert-bullets": "#665c54",
						"--tw-prose-invert-hr": "#504945",
						"--tw-prose-invert-quotes": "#fbf1c7",
						"--tw-prose-invert-quote-borders": "#504945",
						"--tw-prose-invert-captions": "#bdae93",
						"--tw-prose-invert-kbd": "#fbf1c7",
						"--tw-prose-invert-kbd-shadows": hexToRgb("#fbf1c7"),
						"--tw-prose-invert-code": "#fb4934",
						"--tw-prose-invert-pre-code": "#ebdbb2",
						"--tw-prose-invert-pre-bg": "rgb(40 40 40 / 50%)",
						"--tw-prose-invert-th-borders": "#665c54",
						"--tw-prose-invert-td-borders": "#504945",
					},
				},
				solarized: {
					css: {
						// Light theme
						"--tw-prose-body": "#657b83",
						"--tw-prose-headings": "#073642",
						"--tw-prose-lead": "#586e75",
						"--tw-prose-links": "#268bd2",
						"--tw-prose-bold": "#073642",
						"--tw-prose-counters": "#93a1a1",
						"--tw-prose-bullets": "#eee8d5",
						"--tw-prose-hr": "#eee8d5",
						"--tw-prose-quotes": "#073642",
						"--tw-prose-quote-borders": "#eee8d5",
						"--tw-prose-captions": "#93a1a1",
						"--tw-prose-kbd": "#073642",
						"--tw-prose-kbd-shadows": hexToRgb("#073642"),
						"--tw-prose-code": "#dc322f",
						"--tw-prose-pre-code": "#fdf6e3",
						"--tw-prose-pre-bg": "#002b36",
						"--tw-prose-th-borders": "#eee8d5",
						"--tw-prose-td-borders": "#eee8d5",

						// Dark theme
						"--tw-prose-invert-body": "#839496",
						"--tw-prose-invert-headings": "#fdf6e3",
						"--tw-prose-invert-lead": "#93a1a1",
						"--tw-prose-invert-links": "#268bd2",
						"--tw-prose-invert-bold": "#fdf6e3",
						"--tw-prose-invert-counters": "#586e75",
						"--tw-prose-invert-bullets": "#073642",
						"--tw-prose-invert-hr": "#073642",
						"--tw-prose-invert-quotes": "#fdf6e3",
						"--tw-prose-invert-quote-borders": "#073642",
						"--tw-prose-invert-captions": "#586e75",
						"--tw-prose-invert-kbd": "#fdf6e3",
						"--tw-prose-invert-kbd-shadows": hexToRgb("#fdf6e3"),
						"--tw-prose-invert-code": "#cb4b16",
						"--tw-prose-invert-pre-code": "#839496",
						"--tw-prose-invert-pre-bg": "rgb(0 43 54 / 50%)",
						"--tw-prose-invert-th-borders": "#073642",
						"--tw-prose-invert-td-borders": "#073642",
					},
				},
				nord: {
					css: {
						"--tw-prose-body": "#4c566a",
						"--tw-prose-headings": "#2e3440",
						"--tw-prose-lead": "#434c5e",
						"--tw-prose-links": "#5e81ac",
						"--tw-prose-bold": "#2e3440",
						"--tw-prose-counters": "#4c566a",
						"--tw-prose-bullets": "#d8dee9",
						"--tw-prose-hr": "#e5e9f0",
						"--tw-prose-quotes": "#2e3440",
						"--tw-prose-quote-borders": "#e5e9f0",
						"--tw-prose-captions": "#4c566a",
						"--tw-prose-kbd": "#2e3440",
						"--tw-prose-kbd-shadows": hexToRgb("#2e3440"),
						"--tw-prose-code": "#bf616a",
						"--tw-prose-pre-code": "#eceff4",
						"--tw-prose-pre-bg": "#3b4252",
						"--tw-prose-th-borders": "#d8dee9",
						"--tw-prose-td-borders": "#e5e9f0",

						"--tw-prose-invert-body": "#d8dee9",
						"--tw-prose-invert-headings": "#eceff4",
						"--tw-prose-invert-lead": "#e5e9f0",
						"--tw-prose-invert-links": "#88c0d0",
						"--tw-prose-invert-bold": "#eceff4",
						"--tw-prose-invert-counters": "#d8dee9",
						"--tw-prose-invert-bullets": "#4c566a",
						"--tw-prose-invert-hr": "#3b4252",
						"--tw-prose-invert-quotes": "#eceff4",
						"--tw-prose-invert-quote-borders": "#3b4252",
						"--tw-prose-invert-captions": "#d8dee9",
						"--tw-prose-invert-kbd": "#eceff4",
						"--tw-prose-invert-kbd-shadows": hexToRgb("#eceff4"),
						"--tw-prose-invert-code": "#bf616a",
						"--tw-prose-invert-pre-code": "#d8dee9",
						"--tw-prose-invert-pre-bg": "rgb(46 52 64 / 50%)",
						"--tw-prose-invert-th-borders": "#4c566a",
						"--tw-prose-invert-td-borders": "#3b4252",
					},
				},
				monokai: {
					css: {
						"--tw-prose-body": "#272822",
						"--tw-prose-headings": "#171812",
						"--tw-prose-lead": "#3e3d32",
						"--tw-prose-links": "#66d9ef",
						"--tw-prose-bold": "#171812",
						"--tw-prose-counters": "#75715e",
						"--tw-prose-bullets": "#a59f85",
						"--tw-prose-hr": "#f8f8f2",
						"--tw-prose-quotes": "#171812",
						"--tw-prose-quote-borders": "#f8f8f2",
						"--tw-prose-captions": "#75715e",
						"--tw-prose-kbd": "#171812",
						"--tw-prose-kbd-shadows": hexToRgb("#171812"),
						"--tw-prose-code": "#f92672",
						"--tw-prose-pre-code": "#f8f8f2",
						"--tw-prose-pre-bg": "#272822",
						"--tw-prose-th-borders": "#a59f85",
						"--tw-prose-td-borders": "#f8f8f2",

						"--tw-prose-invert-body": "#f8f8f2",
						"--tw-prose-invert-headings": "#f9f8f5",
						"--tw-prose-invert-lead": "#f4f3ec",
						"--tw-prose-invert-links": "#66d9ef",
						"--tw-prose-invert-bold": "#f9f8f5",
						"--tw-prose-invert-counters": "#a59f85",
						"--tw-prose-invert-bullets": "#75715e",
						"--tw-prose-invert-hr": "#49483e",
						"--tw-prose-invert-quotes": "#f9f8f5",
						"--tw-prose-invert-quote-borders": "#49483e",
						"--tw-prose-invert-captions": "#a59f85",
						"--tw-prose-invert-kbd": "#f9f8f5",
						"--tw-prose-invert-kbd-shadows": hexToRgb("#f9f8f5"),
						"--tw-prose-invert-code": "#f92672",
						"--tw-prose-invert-pre-code": "#f8f8f2",
						"--tw-prose-invert-pre-bg": "rgb(39 40 34 / 50%)",
						"--tw-prose-invert-th-borders": "#75715e",
						"--tw-prose-invert-td-borders": "#49483e",
					},
				},
			}),
		},
	},
	plugins: [tailwindcssAnimate, tailwindTypography],
} satisfies Config;
