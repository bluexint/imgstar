import { describe, expect, it } from "vitest";
import {
  buildWafObjectExpression,
  buildWafObjectPattern,
  collectWafSuffixes,
  resolveOrderedWafSuffixes
} from "@/utils/waf";

describe("waf helpers", () => {
  it("collects suffixes from object keys and file names with stable order", () => {
    const suffixes = collectWafSuffixes(
      ["cover.WEBP", "avatar.jpeg", "broken-name", "face.PNG", "face.P NG", "face.P$NG", "face.P=NG"],
      [
        "img/public/000000000.jpg",
        "img/public/000000001.PNG",
        "img/public/000000002.webp",
        "img/public/%2e%2e/etc/passwd"
      ]
    );

    expect(suffixes).toEqual(["jpeg", "jpg", "png", "webp"]);
  });

  it("rejects suffix candidates with whitespace and special tokens", () => {
    expect(resolveOrderedWafSuffixes(["p ng", "p$ng", "p·ng", "^png", "png"])).toEqual(["png"]);
  });

  it("falls back to default suffixes when no valid suffix exists", () => {
    const suffixes = resolveOrderedWafSuffixes(["", null, undefined]);
    expect(suffixes).toEqual(["bmp", "gif", "jpeg", "jpg", "png", "svg", "webp"]);
  });

  it("builds object allowlist pattern and expression", () => {
    const pattern = buildWafObjectPattern([
      "img/public/000000002.webp",
      "/img/public/000000001.png",
      "img/public/000000001.png",
      "/img/public/../img/public/000000006.png",
      "/img//img/public/000000006.png",
      "/img/public/000000006.png;a=1",
      "/img/public/;x=1/000000006.png",
      "img/public/%2e%2e/etc/passwd"
    ], "https://cdn.example.com");
    const expression = buildWafObjectExpression([
      "img/public/000000002.webp",
      "/img/public/000000001.png",
      "img/public/000000001.png",
      "/img/public/../img/public/000000006.png",
      "/img//img/public/000000006.png",
      "/img/public/000000006.png;a=1",
      "/img/public/;x=1/000000006.png",
      "img/public/%2e%2e/etc/passwd"
    ], "https://cdn.example.com");

    expect(pattern).toBe("^(?:/img/public/000000001\\.png|/img/public/000000002\\.webp)$");
    expect(expression).toBe(
      "http.host eq \"cdn.example.com\" and not (raw.http.request.uri.path in {\"/img/public/000000001.png\" \"/img/public/000000002.webp\"})"
    );
  });

  it("compresses consecutive numbered paths into a regex range", () => {
    const pattern = buildWafObjectPattern([
      "img/public/000000001.png",
      "img/public/000000002.png",
      "img/public/000000003.png",
      "img/public/000000005.png"
    ]);

    expect(pattern).toBe(
      "^(?:/img/public/000000005\\.png|/img/public/00000000[1-3]\\.png)$"
    );
  });

  it("scopes object expression by cdn base host and path", () => {
    const pattern = buildWafObjectPattern(
      [
        "img/public/000000002.webp",
        "/img/public/000000001.png"
      ],
      "https://cdn.example.com/media/"
    );
    const expression = buildWafObjectExpression(
      [
        "img/public/000000002.webp",
        "/img/public/000000001.png"
      ],
      "https://cdn.example.com/media/"
    );

    expect(pattern).toBe(
      "^(?:/media/img/public/000000001\\.png|/media/img/public/000000002\\.webp)$"
    );
    expect(expression).toBe(
      "http.host eq \"cdn.example.com\" and not (raw.http.request.uri.path in {\"/media/img/public/000000001.png\" \"/media/img/public/000000002.webp\"})"
    );
  });

  it("filters traversal and encoded bypass object keys from the expression", () => {
    const expression = buildWafObjectExpression([
      "img/public/000000006.png",
      "/img/public/../img/public/000000006.png",
      "/img//img/public/000000006.png",
      "/img/public/000000006.png;a=1",
      "/img/public/000000006.png=1",
      "/img/public/;x=1/000000006.png",
      "img/public/%2e%2e/etc/passwd"
    ], "https://cdn.example.com");

    expect(expression).toContain("http.host eq \"cdn.example.com\"");
    expect(expression).toContain("raw.http.request.uri.path in {");
    expect(expression).toContain("\"/img/public/000000006.png\"");
    expect(expression).not.toContain("../img/public/000000006.png");
    expect(expression).not.toContain("//img/public/000000006.png");
    expect(expression).not.toContain("000000006.png;a=1");
    expect(expression).not.toContain("000000006.png=1");
    expect(expression).not.toContain(";x=1/000000006.png");
    expect(expression).not.toContain("%2e%2e/etc/passwd");
  });
});
