export type FetchErrorName = "NOT_FOUND" | "UNKNOWN" | "ERROR_STATUS";

export class FetchError extends Error {
	constructor(
		public name: FetchErrorName,
		public status?: number,
	) {
		const message =
			typeof ErrorMessages[name] === "function"
				? // biome-ignore lint/style/noNonNullAssertion: <explanation>
					ErrorMessages[name](status!)
				: ErrorMessages[name];
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
};
