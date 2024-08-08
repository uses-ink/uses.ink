import pino from "pino";
import { IS_DEV } from "../constants";

const level = IS_DEV ? "debug" : "info";

export const serverLogger = pino({ level }).child({ type: "server" });
