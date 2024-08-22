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
	if (typeof props.src !== "string") {
		return <img {...props} />;
	}
	const src = new URL(props.src);
	const isBadge = BADGES_HOSTS.includes(src.hostname);

	const hashType: "dark" | "light" | undefined = DARK_HASHES.includes(src.hash)
		? "dark"
		: LIGHT_HASHES.includes(src.hash)
			? "light"
			: undefined;

	return (
		// biome-ignore lint/a11y/useAltText: This will (maybe) be provided by the user
		<img
			className={`${isBadge ? "!inline rounded-none" : ""} ${hashType ? (hashType === "dark" ? "hidden dark:block" : "dark:hidden block") : ""}`}
			{...props}
		/>
	);
};

export default Img;
