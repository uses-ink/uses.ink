import { createConsola, LogLevels } from "consola";
import { IS_DEV } from "../constants";

const level = IS_DEV ? LogLevels.debug : LogLevels.info;

export const clientLogger = createConsola({ level }).withTag("client");
