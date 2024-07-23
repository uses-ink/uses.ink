import ThemeSelect from "./theme-select";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "./ui/tooltip";
import { BUILD_DATE, BUILD_HASH } from "@/lib/constants";
import moment from "moment";

export const Footer = () => {
	return (
		<div className="flex justify-center mb-12 text-xs">
			<div className="flex gap-4 items-center justify-between">
				<ThemeSelect />
				<TooltipProvider>
					<Tooltip>
						<TooltipTrigger>
							<p className="whitespace-nowrap !text-muted-foreground">
								Made with{" "}
								<a href="https://uses.ink" target="_blank" rel="noreferrer">
									uses.ink
								</a>
							</p>
						</TooltipTrigger>
						<TooltipContent>
							Built on {moment(BUILD_DATE).format("LLL")}{" "}
							<span className="dark:text-gray-500 text-gray-300">
								({BUILD_HASH.slice(0, 7)})
							</span>
						</TooltipContent>
					</Tooltip>
				</TooltipProvider>
			</div>
		</div>
	);
};
