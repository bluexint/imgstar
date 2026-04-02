const DEFAULT_WAF_SUFFIXES = ["bmp", "gif", "jpeg", "jpg", "png", "svg", "webp"] as const;

const toSortedUnique = (values: Iterable<string>): string[] =>
  Array.from(new Set(values)).sort((left, right) => left.localeCompare(right));

const containsWafBypassTokens = (value: string): boolean => {
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0;
    if (
      codePoint <= 0x1f ||
      codePoint === 0x7f ||
      /\s/u.test(character) ||
      codePoint > 0x7f ||
      "^$;%?#=".includes(character)
    ) {
      return true;
    }
  }

  const normalized = value.replace(/\\/g, "/");
  return (
    normalized.includes("//") ||
    normalized.includes("/./") ||
    normalized.includes("/../") ||
    normalized.startsWith("./") ||
    normalized.startsWith("../") ||
    normalized.endsWith("/.") ||
    normalized.endsWith("/..") ||
    normalized === "." ||
    normalized === ".."
  );
};

export const normalizeWafSuffix = (value?: string | null): string | undefined => {
  if (!value) {
    return undefined;
  }

  const cleaned = value.trim().replace(/^\.+/, "");

  if (
    cleaned.length === 0 ||
    cleaned.length > 16 ||
    containsWafBypassTokens(cleaned) ||
    !/^[a-zA-Z0-9]+$/.test(cleaned)
  ) {
    return undefined;
  }

  return cleaned.toLowerCase();
};

export const extractSuffixFromFileName = (
  fileName?: string | null
): string | undefined => {
  if (!fileName) {
    return undefined;
  }

  const dotIndex = fileName.lastIndexOf(".");
  if (dotIndex < 0 || dotIndex === fileName.length - 1) {
    return undefined;
  }

  return normalizeWafSuffix(fileName.slice(dotIndex + 1));
};

export const extractSuffixFromObjectKey = (
  objectKey?: string | null
): string | undefined => {
  if (!objectKey) {
    return undefined;
  }

  const normalizedKey = normalizeWafObjectKey(objectKey);
  if (!normalizedKey) {
    return undefined;
  }

  const fileName = normalizedKey.split("/").pop() ?? normalizedKey;
  return extractSuffixFromFileName(fileName);
};

export const resolveOrderedWafSuffixes = (
  candidates: Array<string | undefined | null>
): string[] => {
  const normalized = toSortedUnique(
    candidates
      .map((value) => normalizeWafSuffix(value))
      .filter((value): value is string => Boolean(value))
  );

  if (normalized.length > 0) {
    return normalized;
  }

  return [...DEFAULT_WAF_SUFFIXES];
};

export const collectWafSuffixes = (
  fileNames: Array<string | undefined | null>,
  objectKeys: Array<string | undefined | null>
): string[] => {
  const fromObjectKeys = objectKeys
    .map((value) => extractSuffixFromObjectKey(value))
    .filter((value): value is string => Boolean(value));

  const fromFileNames = fileNames
    .map((value) => extractSuffixFromFileName(value))
    .filter((value): value is string => Boolean(value));

  return resolveOrderedWafSuffixes([...fromObjectKeys, ...fromFileNames]);
};

export const buildWafPattern = (
  digitCount: number,
  suffixes: Array<string | undefined | null>
): string => {
  const orderedSuffixes = resolveOrderedWafSuffixes(suffixes);
  const suffixGroup = orderedSuffixes.join("|");

  return String.raw`^/img/public/[0-9]{${digitCount}}\.(?:${suffixGroup})$`;
};

interface WafCdnScope {
  scheme?: string;
  host?: string;
  pathPrefix: string;
}

interface WafObjectKeyParts {
  prefix: string;
  number: string;
  suffix: string;
}

interface WafObjectGroup {
  prefix: string;
  suffix: string;
  width: number;
  numbers: string[];
}

const wafRegexMeta = /[.*+?^${}()|[\]\\]/g;

const escapeWafRegex = (value: string): string => value.replace(wafRegexMeta, "\\$&");

const buildDigitRange = (start: number, end: number): string =>
  start === end ? `${start}` : `[${start}-${end}]`;

const parseWafObjectKeyParts = (
  value?: string | null,
  pathPrefix = ""
): WafObjectKeyParts | undefined => {
  const normalized = normalizeWafObjectKey(value, pathPrefix);
  if (!normalized) {
    return undefined;
  }

  const lastSlash = normalized.lastIndexOf("/");
  const lastDot = normalized.lastIndexOf(".");
  if (lastSlash < 0 || lastDot <= lastSlash || lastDot === normalized.length - 1) {
    return undefined;
  }

  const number = normalized.slice(lastSlash + 1, lastDot);
  const suffix = normalized.slice(lastDot + 1);

  if (!/^\d+$/.test(number)) {
    return undefined;
  }

  return {
    prefix: normalized.slice(0, lastSlash + 1),
    number,
    suffix
  };
};

const buildNumericRangeRegex = (start: string, end: string): string => {
  if (start === end) {
    return start;
  }

  let prefixLength = 0;
  while (prefixLength < start.length && start[prefixLength] === end[prefixLength]) {
    prefixLength += 1;
  }

  const prefix = start.slice(0, prefixLength);
  const startDigit = Number(start[prefixLength]);
  const endDigit = Number(end[prefixLength]);
  const remainingLength = start.length - prefixLength - 1;

  if (remainingLength === 0) {
    return `${prefix}${buildDigitRange(startDigit, endDigit)}`;
  }

  const parts = [
    buildNumericRangeRegex(start, `${prefix}${start[prefixLength]}${"9".repeat(remainingLength)}`)
  ];

  if (startDigit + 1 <= endDigit - 1) {
    parts.push(
      `${prefix}${buildDigitRange(startDigit + 1, endDigit - 1)}[0-9]{${remainingLength}}`
    );
  }

  parts.push(
    buildNumericRangeRegex(
      `${prefix}${end[prefixLength]}${"0".repeat(remainingLength)}`,
      end
    )
  );

  const uniqueParts = Array.from(new Set(parts));
  return uniqueParts.length === 1 ? uniqueParts[0] : `(?:${uniqueParts.join("|")})`;
};

const normalizeWafPathPrefix = (value: string): string => {
  const cleaned = value
    .trim()
    .replace(/\\/g, "/")
    .replace(/\/+$/, "");

  if (cleaned.length === 0 || cleaned === "/") {
    return "";
  }

  return cleaned.startsWith("/") ? cleaned : `/${cleaned}`;
};

const buildWafGuardedPathPrefix = (pathPrefix: string): string =>
  pathPrefix ? `${pathPrefix}/img/public/` : "/img/public/";

const resolveWafCdnScope = (cdnBaseUrl?: string | null): WafCdnScope => {
  const raw = cdnBaseUrl?.trim();
  if (!raw) {
    return { pathPrefix: "" };
  }

  try {
    const parsed = new URL(raw);
    return {
      scheme: parsed.protocol.replace(/:$/, "").toLowerCase(),
      host: parsed.hostname.toLowerCase(),
      pathPrefix: normalizeWafPathPrefix(parsed.pathname)
    };
  } catch {
    return { pathPrefix: "" };
  }
};

const buildWafFullUriPrefix = (scope: WafCdnScope): string | undefined => {
  if (!scope.scheme || !scope.host) {
    return undefined;
  }

  return `${scope.scheme}://${scope.host}${buildWafGuardedPathPrefix(scope.pathPrefix)}`;
};

const isStrictAllowlistPath = (path: string, guardedPrefix: string): boolean => {
  if (!path.startsWith(guardedPrefix)) {
    return false;
  }

  const filePart = path.slice(guardedPrefix.length);
  if (filePart.length === 0 || filePart.includes("/") || containsWafBypassTokens(filePart)) {
    return false;
  }

  const dotIndex = filePart.lastIndexOf(".");
  if (dotIndex <= 0 || dotIndex === filePart.length - 1) {
    return false;
  }

  const number = filePart.slice(0, dotIndex);
  const suffix = filePart.slice(dotIndex + 1);

  if (!/^\d+$/.test(number) || number.length > 20) {
    return false;
  }

  if (!/^[a-zA-Z0-9]+$/.test(suffix) || suffix.length > 16) {
    return false;
  }

  return true;
};

const buildWafSetLiteral = (values: string[]): string =>
  `{${values.map((value) => quoteWafString(value)).join(" ")}}`;

const buildWafFullUri = (scope: WafCdnScope, normalizedPath: string): string =>
  `${scope.scheme}://${scope.host}${normalizedPath}`;

const collectWafAllowlistFullUris = (
  objectKeys: Array<string | undefined | null>,
  cdnBaseUrl?: string | null
): string[] => {
  const scope = resolveWafCdnScope(cdnBaseUrl);
  const guardedPrefix = buildWafGuardedPathPrefix(scope.pathPrefix);
  const uniqueUris = new Set<string>();

  for (const value of objectKeys) {
    const normalized = normalizeWafObjectKey(value, scope.pathPrefix);
    if (normalized && isStrictAllowlistPath(normalized, guardedPrefix) && scope.scheme && scope.host) {
      uniqueUris.add(buildWafFullUri(scope, normalized));
    }
  }

  return toSortedUnique(uniqueUris);
};

const collectWafAllowlistPaths = (
  objectKeys: Array<string | undefined | null>,
  cdnBaseUrl?: string | null
): string[] => {
  const scope = resolveWafCdnScope(cdnBaseUrl);
  const guardedPrefix = buildWafGuardedPathPrefix(scope.pathPrefix);
  const uniquePaths = new Set<string>();

  for (const value of objectKeys) {
    const normalized = normalizeWafObjectKey(value, scope.pathPrefix);
    if (normalized && isStrictAllowlistPath(normalized, guardedPrefix)) {
      uniquePaths.add(normalized);
    }
  }

  return toSortedUnique(uniquePaths);
};

const normalizeWafObjectKey = (
  value?: string | null,
  pathPrefix = ""
): string | undefined => {
  if (!value) {
    return undefined;
  }

  const trimmed = value.trim();
  if (!trimmed || containsWafBypassTokens(trimmed)) {
    return undefined;
  }

  const cleaned = trimmed
    .replace(/\\/g, "/")
    .replace(/^\/+/, "");

  if (cleaned.length === 0 || containsWafBypassTokens(cleaned)) {
    return undefined;
  }

  const normalizedPath = `/${cleaned}`;
  if (!pathPrefix) {
    return normalizedPath;
  }

  return `${pathPrefix}${normalizedPath}`;
};

const quoteWafString = (value: string): string => JSON.stringify(value);

const collectWafObjectFragments = (
  objectKeys: Array<string | undefined | null>,
  cdnBaseUrl?: string | null
): string[] => {
  const scope = resolveWafCdnScope(cdnBaseUrl);
  const grouped = new Map<string, WafObjectGroup>();

  for (const value of objectKeys) {
    const parts = parseWafObjectKeyParts(value, scope.pathPrefix);
    if (!parts) {
      continue;
    }

    const groupKey = `${parts.prefix}\u0000${parts.suffix}\u0000${parts.number.length}`;
    const existing = grouped.get(groupKey);
    if (existing) {
      existing.numbers.push(parts.number);
      continue;
    }

    grouped.set(groupKey, {
      prefix: parts.prefix,
      suffix: parts.suffix,
      width: parts.number.length,
      numbers: [parts.number]
    });
  }

  const fragments: string[] = [];

  for (const group of grouped.values()) {
    const orderedNumbers = toSortedUnique(group.numbers);
    if (orderedNumbers.length === 0) {
      continue;
    }

    let runStart = orderedNumbers[0];
    let previous = orderedNumbers[0];

    for (const current of orderedNumbers.slice(1)) {
      if (BigInt(current) === BigInt(previous) + 1n) {
        previous = current;
        continue;
      }

      fragments.push(
        `${escapeWafRegex(group.prefix)}${buildNumericRangeRegex(runStart, previous)}\\.${escapeWafRegex(group.suffix)}`
      );
      runStart = current;
      previous = current;
    }

    fragments.push(
      `${escapeWafRegex(group.prefix)}${buildNumericRangeRegex(runStart, previous)}\\.${escapeWafRegex(group.suffix)}`
    );
  }

  fragments.sort();
  return fragments;
};

export const buildWafObjectPattern = (
  objectKeys: Array<string | undefined | null>,
  cdnBaseUrl?: string | null
): string => {
  const fragments = collectWafObjectFragments(objectKeys, cdnBaseUrl);

  if (fragments.length === 0) {
    return "";
  }

  if (fragments.length === 1) {
    return `^${fragments[0]}$`;
  }

  return `^(?:${fragments.join("|")})$`;
};

export const buildWafObjectExpression = (
  objectKeys: Array<string | undefined | null>,
  cdnBaseUrl?: string | null
): string => {
  const scope = resolveWafCdnScope(cdnBaseUrl);
  const allowlistedPaths = collectWafAllowlistPaths(objectKeys, cdnBaseUrl);

  if (scope.host) {
    const hostCondition = `http.host eq ${quoteWafString(scope.host)}`;

    if (allowlistedPaths.length === 0) {
      return hostCondition;
    }

    return `${hostCondition} and not (raw.http.request.uri.path in ${buildWafSetLiteral(allowlistedPaths)})`;
  }

  const pathPrefix = buildWafGuardedPathPrefix(scope.pathPrefix);
  const pathCondition = `starts_with(http.request.uri.path, ${quoteWafString(pathPrefix)})`;

  if (allowlistedPaths.length === 0) {
    return pathCondition;
  }

  return `${pathCondition} and not (raw.http.request.uri.path in ${buildWafSetLiteral(allowlistedPaths)})`;
};

export const defaultWafSuffixes = [...DEFAULT_WAF_SUFFIXES];
