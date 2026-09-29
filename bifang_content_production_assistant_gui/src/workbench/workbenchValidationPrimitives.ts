/** IPC 严格解码过程中使用的普通 JSON 对象。 */
export type JsonRecord = Record<string, unknown>;

/** Tauri command 拒绝时允许前端展示的稳定安全错误。 */
export interface CommandError {
  code: string;
  message: string;
}

/** 判断未知 IPC 值是否为普通 JSON 对象。 */
export function asRecord(value: unknown, path: string): JsonRecord {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw invalid(path);
  }
  return value as JsonRecord;
}

/** 要求对象字段与 Rust 序列化契约完全一致。 */
export function exactRecord(
  value: unknown,
  keys: readonly string[],
  path: string,
): JsonRecord {
  const record = asRecord(value, path);
  const actual = Object.keys(record).sort();
  const expected = [...keys].sort();
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== expected[index])
  ) {
    throw invalid(path);
  }
  return record;
}

/** 创建不回显外部数据的稳定 IPC 校验异常。 */
export function invalid(path: string): Error {
  return new Error(`invalid workbench IPC response at ${path}`);
}

/** 递归按键排序，避免仅因 JSON 对象属性顺序不同而误判原子保存响应。 */
export function canonicalJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map((item) => canonicalJson(item));
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value as JsonRecord)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, nested]) => [key, canonicalJson(nested)]),
  );
}

/** 读取字符串；仅协议标识和名称要求非空。 */
export function stringValue(
  value: unknown,
  path: string,
  nonEmpty = false,
): string {
  if (typeof value !== "string" || (nonEmpty && value.trim().length === 0)) {
    throw invalid(path);
  }
  return value;
}

/** 读取 JSON 可安全表达的非负整数。 */
export function uintValue(
  value: unknown,
  path: string,
  positive = false,
): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < (positive ? 1 : 0)
  ) {
    throw invalid(path);
  }
  return value;
}

/** 读取布尔字段。 */
export function booleanValue(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw invalid(path);
  return value;
}

/** 读取受限枚举字段。 */
export function enumValue(
  value: unknown,
  allowed: Set<string>,
  path: string,
): string {
  const decoded = stringValue(value, path, true);
  if (!allowed.has(decoded)) throw invalid(path);
  return decoded;
}

/** 读取可空字段，并对非空值执行指定解码器。 */
export function nullable<T>(
  value: unknown,
  decoder: (input: unknown, path: string) => T,
  path: string,
): T | null {
  return value === null ? null : decoder(value, path);
}

/** 读取数组，并逐项验证而非仅验证容器。 */
export function arrayValue<T>(
  value: unknown,
  decoder: (input: unknown, path: string) => T,
  path: string,
): T[] {
  if (!Array.isArray(value)) throw invalid(path);
  return value.map((item, index) => decoder(item, `${path}[${index}]`));
}

/** 读取普通字符串数组。 */
export function stringArray(value: unknown, path: string): string[] {
  return arrayValue(
    value,
    (item, itemPath) => stringValue(item, itemPath),
    path,
  );
}
