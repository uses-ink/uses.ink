import { createConsola, LogLevels } from "consola";
import { IS_DEV } from "../constants";

const level = IS_DEV ? LogLevels.debug : LogLevels.info;

export const serverLogger = createConsola({ level }).withTag("server");
