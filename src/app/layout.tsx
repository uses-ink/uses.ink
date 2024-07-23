import type { Metadata } from "next";
import { ThemeProvider } from "next-themes";
import { Inter } from "next/font/google";
import "./globals.css";

const inter = Inter({ subsets: ["latin"] });

export const metadata: Metadata = {
	title: "uses.ink",
	description: "You got a github repo? You got a blog.",
	openGraph: {
		type: "website",
		url: "https://uses.ink",
		title: "uses.ink",
		description: "You got a github repo? You got a blog.",
		images: [
			{
				url: "https://uses.ink/og.png",
				width: 1277,
				height: 641,
				alt: "uses.ink",
			},
		],
	},
};

export default function RootLayout({
	children,
}: Readonly<{
	children: React.ReactNode;
}>) {
	return (
		<html lang="en" suppressHydrationWarning>
			<body className={inter.className}>
				<ThemeProvider attribute="class">{children}</ThemeProvider>
			</body>
		</html>
	);
}
