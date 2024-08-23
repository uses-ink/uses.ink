export function throwErrorWithHint(message: string): never {
	throw new Error(
		`${message}\nSee the error report above for more informations.\n\nIf you believe this is a bug, please file an issue at https://github.com/HiDeoo/-d2/issues/new/choose`,
	);
}
