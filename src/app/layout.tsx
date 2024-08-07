import type { Metadata } from "next";
import { ThemeProvider } from "next-themes";
import { Inter } from "next/font/google";
import "@/styles/globals.css";

const inter = Inter({ subsets: ["latin"] });

// export const metadata: Metadata = {
// 	title: "uses.ink",
// 	description: "You got a github repo? You got a blog.",
// 	openGraph: {
// 		type: "website",
// 		url: "https://uses.ink",
// 		title: "uses.ink",
// 		description: "You got a github repo? You got a blog.",
// 		images: [
// 			{
// 				url: "https://uses.ink/og.png",
// 				width: 1277,
// 				height: 641,
// 				alt: "uses.ink",
// 			},
// 		],
// 	},
// };

export default function RootLayout({
	children,
}: Readonly<{
	children: React.ReactNode;
}>) {
	return (
		<html lang="en" suppressHydrationWarning>
			<head>
				<link rel="preconnect" href="https://fonts.googleapis.com" />
				<link
					rel="preconnect"
					href="https://fonts.gstatic.com"
					crossOrigin="anonymous"
				/>
				<link
					href="https://fonts.googleapis.com/css2?family=Fuzzy+Bubbles:wght@400;700&family=Source+Sans+3:ital,wght@0,200..900;1,200..900&display=swap"
					rel="stylesheet"
				/>
			</head>
			<body className={inter.className}>
				<ThemeProvider attribute="class">{children}</ThemeProvider>
			</body>
		</html>
	);
}
