import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export const isErrorHasStatus = (
	raw: unknown,
): raw is {
	status: number;
} => {
	if (typeof raw !== "object") return false;
	if (raw === null) throw raw;
	if ("status" in raw) {
		return typeof (raw as { status: number }).status === "number";
	}
	return false;
};
