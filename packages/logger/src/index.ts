import { createConsola, LogLevels } from "consola";
import { IS_DEV } from "@uses.ink/constants";

const level = IS_DEV ? LogLevels.debug : LogLevels.info;

export const serverLogger = createConsola({ level }).withTag("server");

export const clientLogger = createConsola({ level }).withTag("client");
