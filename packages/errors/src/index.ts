export type FetchErrorName =
	| "NOT_FOUND"
	| "UNKNOWN"
	| "ERROR_STATUS"
	| "NO_TREE";

export class FetchError extends Error {
	constructor(
		public name: FetchErrorName,
		...args: any[]
	) {
		const message =
			typeof ErrorMessages[name] === "function"
				? (ErrorMessages[name] as (...args: any) => string)(...args)
				: (ErrorMessages[name] as string);
		super(message);
		this.message = message;
		this.name = name;
	}
}

const ErrorMessages: Record<
	FetchErrorName,
	string | ((...args: any) => string)
> = {
	NOT_FOUND: (path) => `Not found${path ? `: ${path}` : ""}`,
	NO_TREE: (repo) => `No tree found${repo ? `: ${repo}` : ""}`,
	UNKNOWN: "Unknown error",
	ERROR_STATUS: (status) => `Error status ${status}`,
};
