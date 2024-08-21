import type { z } from "zod";
import type * as schemas from "@uses.ink/schemas";

export type D2DiagramAttributes = z.infer<typeof schemas.D2AttributesSchema>;
export type D2UserConfig = z.input<typeof schemas.D2ConfigSchema>;
export type D2Config = z.output<typeof schemas.D2ConfigSchema>;
export type MathEngine = z.infer<typeof schemas.MathEngineSchema>;
export type Layout = z.infer<typeof schemas.LayoutSchema>;
export type RepoConfig = z.infer<typeof schemas.RepoConfigSchema>;
export type Meta = z.infer<typeof schemas.MetaSchema>;
export type UserConfig = z.input<typeof schemas.UserConfigSchema>;
