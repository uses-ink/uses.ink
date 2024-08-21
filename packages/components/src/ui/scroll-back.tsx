import { ChevronUp } from "lucide-react";
import { cn } from "../lib/utils";
import { useState, useEffect, useCallback } from "react";

export const ScrollBack = () => {
	const [canScroll, setCanScroll] = useState(false);

	useEffect(() => {
		const handleScroll = () => {
			setCanScroll(window.scrollY > 100);
		};

		window.addEventListener("scroll", handleScroll);

		return () => {
			window.removeEventListener("scroll", handleScroll);
		};
	}, []);

	const scrollUp = useCallback(() => {
		window.scrollTo({ top: 0, behavior: "smooth" });
	}, []);

	return (
		<div className="fixed sm:bottom-8 sm:right-8 bottom-4 right-4">
			<ChevronUp
				className={cn(
					"h-4 w-4 sm:w-6 sm:h-6 cursor-pointer transition-opacity ease-in-out duration-300",
					{
						"opacity-100": canScroll,
						"opacity-0": !canScroll,
					},
				)}
				onClick={scrollUp}
			/>
		</div>
	);
};
