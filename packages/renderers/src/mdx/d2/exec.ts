import { spawn } from "node:child_process";

export function exec(
	command: string,
	args: string[],
	stdin?: string,
	cwd?: string,
) {
	return new Promise<string[]>((resolve, reject) => {
		const child = spawn(command, args, {
			stdio: [],
		});

		const cleanup = () => {
			child.off("error", onError);
			child.off("close", onClose);
			child.stdout.off("data", onData);
		};

		const output: string[] = [];
		const errorMessage = `Unable to run command: '${command} ${args.join(" ")}'.`;
		const onData = (data: Buffer) => {
			const lines = data
				.toString()
				.split("\n")
				.filter((line) => line.length > 0);

			output.push(...lines);
		};
		child.stdout.on("data", onData);

		const onError = (data: Buffer) => {
			cleanup();
			reject(new Error(errorMessage, { cause: data.toString() }));
		};
		child.on("error", onError);

		const onClose = (code: number) => {
			cleanup();
			if (code !== 0) {
				return reject(new Error(errorMessage));
			}
			resolve(output);
		};
		child.on("close", onClose);

		if (stdin) {
			child.stdin.write(stdin);
			child.stdin.end();
		}
	});
}
