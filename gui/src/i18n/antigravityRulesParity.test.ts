import { describe, it, expect } from "vitest";
import { translations as en } from "./lang/en";
import { translations as ja } from "./lang/ja";
import { translations as zhCN } from "./lang/zh-CN";
import { translations as zhTW } from "./lang/zh-TW";
import { translations as de } from "./lang/de";
import { translations as fr } from "./lang/fr";
import { translations as es } from "./lang/es";
import { translations as ko } from "./lang/ko";

const ALL_LOCALES = [
  { code: "en", map: en },
  { code: "zh-CN", map: zhCN },
  { code: "zh-TW", map: zhTW },
  { code: "de", map: de },
  { code: "fr", map: fr },
  { code: "es", map: es },
  { code: "ko", map: ko },
];

describe("Antigravity Rules i18n Key Parity", () => {
  const jaKeys = Object.keys(ja).filter((k) => k.startsWith("antigravityRules."));

  it("should have antigravityRules keys defined in ja source", () => {
    expect(jaKeys.length).toBeGreaterThan(0);
  });

  ALL_LOCALES.forEach(({ code, map }) => {
    it(`should have complete key parity and non-empty strings in ${code}`, () => {
      const targetMap = map as Record<string, string>;
      const missingKeys: string[] = [];
      const emptyKeys: string[] = [];

      for (const key of jaKeys) {
        if (!(key in targetMap)) {
          missingKeys.push(key);
        } else if (typeof targetMap[key] !== "string" || targetMap[key].trim().length === 0) {
          emptyKeys.push(key);
        }
      }

      expect(missingKeys, `Missing keys in ${code}: ${missingKeys.join(", ")}`).toHaveLength(0);
      expect(emptyKeys, `Empty keys in ${code}: ${emptyKeys.join(", ")}`).toHaveLength(0);
    });
  });
});
