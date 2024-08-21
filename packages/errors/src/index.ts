export type FetchErrorName =
	| "NOT_FOUND"
	| "UNKNOWN"
	| "ERROR_STATUS"
	| "NOT_FOUND_PREFETCH";

export class FetchError extends Error {
	constructor(name: FetchErrorName, ...args: any[]) {
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
	string | ((status: number) => string)
> = {
	NOT_FOUND: "File not found",
	UNKNOWN: "Unknown error",
	ERROR_STATUS: (status) => `Error status ${status}`,
	NOT_FOUND_PREFETCH: "Not found during prefetch",
};
