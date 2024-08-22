import { Moon, Sun } from "lucide-react";

import { Button } from "@uses.ink/components/ui/button";
export function ModeToggle() {
	return (
		<Button
			variant="ghost"
			size="icon"
			className="border-none"
			id="mode-toggle"
		>
			<Sun className="h-[1.2rem] w-[1.2rem] rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
			<Moon className="absolute h-[1.2rem] w-[1.2rem] rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
			<span className="sr-only">Toggle theme</span>
		</Button>
	);
}
