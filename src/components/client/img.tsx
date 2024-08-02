"use client";

import { useTheme } from "next-themes";
import { useEffect, useState } from "react";

const BADGES_HOSTS = ["img.shields.io", "badgen.net", "forthebadge.com"];
const DARK_HASHES = ["#dark", "#dark-mode", "#darkmode", "#gh-dark-mode-only"];
const LIGHT_HASHES = [
	"#light",
	"#light-mode",
	"#lightmode",
	"#gh-light-mode-only",
];

const Img = (props: any) => {
	const src = new URL(props.src);
	const isBadge = BADGES_HOSTS.includes(src.hostname);
	const { resolvedTheme } = useTheme();
	const themeRestriction = src.hash
		? (resolvedTheme === "dark" ? DARK_HASHES : LIGHT_HASHES).includes(src.hash)
		: undefined;
	const [isLoaded, setIsLoaded] = useState(false);

	useEffect(() => {
		setIsLoaded(true);
	}, []);

	return (
		// biome-ignore lint/a11y/useAltText: This will (maybe) be provided by the user
		<img
			className={`${isBadge ? "!inline" : ""} ${(themeRestriction === undefined ? false : !themeRestriction || !isLoaded) ? "hidden" : ""}`}
			{...props}
		/>
	);
};

export default Img;
