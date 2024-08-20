"use client";

import { BADGES_HOSTS, DARK_HASHES, LIGHT_HASHES } from "@uses.ink/constants";
import { useTheme } from "next-themes";
import { useEffect, useState } from "react";



const Img = (props: any) => {
	if (typeof props.src !== "string") {
		return <img {...props} />;
	}
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
