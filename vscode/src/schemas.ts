import { z, ZodType } from "zod";

export const zAliveMessage = z.object({ status: z.literal(true) });

export const zIndex = z.number().int();
export const zLoc = z.number().int();
export const zRange = z.object({ from: zLoc, until: zLoc });
export const zMirDecl = z.union([
  z.object({
    type: z.literal("user"),
    local_index: zIndex,
    name: z.string(),
    span: zRange,
    ty: z.string(),
    lives: zRange.array(),
    must_live_at: zRange.array(),
    drop: z.boolean(),
    drop_range: zRange.array(),
    //can_live_at: zRange.array(),
  }),
  z.object({
    type: z.literal("other"),
    local_index: zIndex,
    ty: z.string(),
    lives: zRange.array(),
    must_live_at: zRange.array(),
    drop: z.boolean(),
    drop_range: zRange.array(),
    //can_live_at: zRange.array(),
  }),
]);
export const zMirRval = z.union([
  z.object({
    type: z.literal("move"),
    target_local_index: zIndex,
    range: zRange,
  }),
  z.object({
    type: z.literal("borrow"),
    target_local_index: zIndex,
    range: zRange,
    mutable: z.boolean(),
    outlive: zRange.nullish(),
  }),
]);
export const zMirStatement = z.union([
  z.object({
    type: z.literal("storage_live"),
    target_local_index: zIndex,
    range: zRange,
  }),
  z.object({
    type: z.literal("storage_dead"),
    target_local_index: zIndex,
    range: zRange,
  }),
  z.object({
    type: z.literal("assign"),
    target_local_index: zIndex,
    range: zRange,
    rval: zMirRval.nullish(),
  }),
]);
export const zMirTerminator = z.union([
  z.object({ type: z.literal("drop"), local_index: zIndex, range: zRange }),
  z.object({
    type: z.literal("call"),
    destination_local_index: zIndex,
    fn_span: zRange,
  }),
  z.object({ type: z.literal("other") }),
]);
export const zMirBasicBlock = z.object({
  statements: z.array(zMirStatement),
  terminator: zMirTerminator.nullish(),
});
export const zMir = z.object({
  basic_blocks: z.array(zMirBasicBlock),
  decls: z.array(zMirDecl),
});
export const zItem = z.object({
  type: z.literal("function"),
  span: zRange,
  mir: zMir,
});
export const zCollectedData = z.object({ items: z.array(zMir) });
export const zWorkspace = z.record(z.string(), zCollectedData);
export const zAnalyzeSuccess = z.object({
  success: z.literal(true),
  compile_error: z.boolean(),
  collected: zCollectedData,
});
export const zAnalyzeResponse = z.union([
  zAnalyzeSuccess,
  z.object({ success: z.literal(false), cause: z.string() }),
]);

export type zInfer<T extends ZodType> = z.infer<T>;

export const zLspLocation = z.object({
  line: z.number().int(),
  character: z.number().int(),
});
export const zLspRange = z.object({ start: zLspLocation, end: zLspLocation });
export const zLspType = z.union([
  z.literal("lifetime"),
  z.literal("imm_borrow"),
  z.literal("mut_borrow"),
  z.literal("move"),
  z.literal("call"),
  z.literal("shared_mut"),
  z.literal("outlive"),
]);
export const zLspCursorResponse = z.object({
  is_analyzed: z.boolean(),
  status: z.union([
    z.literal("analyzing"),
    z.literal("finished"),
    z.literal("error"),
  ]),
  decorations: z
    .object({
      type: zLspType,
      range: zLspRange,
      hover_text: z.string().nullish(),
      overlapped: z.boolean(),
    })
    .array(),
});
