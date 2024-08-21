import { createConsola, LogLevels } from "consola";

export const logger = createConsola({
	level: import.meta.env.DEV ? LogLevels.debug : LogLevels.info,
}).withTag("server");
