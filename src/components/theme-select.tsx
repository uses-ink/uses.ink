"use client";

import { Moon, Sun } from "lucide-react";
import { useTheme } from "next-themes";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { useState, useEffect } from "react";

export default function ThemeSelect() {
	const { resolvedTheme, setTheme } = useTheme();
	const [mounted, setMounted] = useState(false);
	useEffect(() => setMounted(true), []);
	return (
		<DropdownMenu>
			<DropdownMenuTrigger>
				<div className="cursor-pointer">
					{mounted && resolvedTheme === "dark" ? (
						<Moon className="w-4 h-4" />
					) : (
						<Sun className="w-4 h-4" />
					)}
				</div>
			</DropdownMenuTrigger>
			<DropdownMenuContent className="border-muted">
				<DropdownMenuItem onClick={() => setTheme("dark")}>
					Dark
				</DropdownMenuItem>
				<DropdownMenuItem onClick={() => setTheme("light")}>
					Light
				</DropdownMenuItem>
				<DropdownMenuItem onClick={() => setTheme("system")}>
					System
				</DropdownMenuItem>
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
