import { logger } from "@uses.ink/server-logger";
import type { D2Config, D2DiagramAttributes, D2Size } from "@uses.ink/types";
import { exec } from "./exec";

const viewBoxRegex = /viewBox="\d+ \d+ (?<width>\d+) (?<height>\d+)"/;

export async function isD2Installed() {
	try {
		await getD2Version();

		return true;
	} catch {
		return false;
	}
}

export async function generateD2Diagram(
	config: D2Config,
	attributes: D2DiagramAttributes,
	input: string,
	dark: boolean,
) {
	const extraArgs: string[] = [];

	if (attributes.animateInterval) {
		extraArgs.push(`--animate-interval=${attributes.animateInterval}`);
	}

	if (attributes.target !== undefined) {
		extraArgs.push(`--target='${attributes.target}'`);
	}

	try {
		// The `-` argument is used to read from stdin instead of a file.
		const start = Date.now();
		const res = await exec(
			"d2",
			[
				`--layout=${config.layout}`,
				`--theme=${dark ? "201" : "1"}`,
				`--sketch=${attributes.sketch ?? config.sketch}`,
				`--pad=${attributes.pad ?? config.pad}`,
				...extraArgs,
				"-",
				"-",
			],
			input,
		);
		logger.info("Generated D2 diagram in", Date.now() - start, "ms");
		return {
			size: await getD2DiagramSize(res.join("\n")),
			svg: res.join("\n"),
		};
	} catch (error) {
		throw new Error("Failed to generate D2 diagram.", { cause: error });
	}
}

export async function getD2DiagramSize(content: string): Promise<D2Size> {
	try {
		const match = content.match(viewBoxRegex);
		const { height, width } = match?.groups ?? {};

		if (!height || !width) {
			return;
		}

		const computedHeight = Number.parseInt(height, 10);
		const computedWidth = Number.parseInt(width, 10);

		return { height: computedHeight, width: computedWidth };
	} catch (error) {
		throw new Error("Failed to get D2 diagram size", {
			cause: error,
		});
	}
}

async function getD2Version() {
	try {
		const [version] = await exec("d2", ["--version"]);

		if (!version || !/^v?\d+\.\d+\.\d+/.test(version)) {
			throw new Error(`Invalid D2 version, got '${version}'.`);
		}

		return version;
	} catch (error) {
		throw new Error("Failed to get D2 version.", { cause: error });
	}
}
