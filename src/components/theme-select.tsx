"use client";

import { useTheme } from "next-themes";

export default function ThemeSelect() {
	const { theme, setTheme } = useTheme();
	return (
		<select
			value={theme}
			onChange={(e) => setTheme(e.target.value)}
			className="cursor-pointer"
		>
			<option value="system">System</option>
			<option value="light">Light</option>
			<option value="dark">Dark</option>
		</select>
	);
}
