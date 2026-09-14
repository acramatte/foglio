import { describe, expect, it } from "vitest";
import { appearanceLabel, resolveAppearance, systemAppearance } from "./appearance";

describe("appearance resolution", () => {
  it("defaults System to light when no platform preference is available", () => {
    expect(systemAppearance(undefined)).toBe("light");
    expect(resolveAppearance("system", null)).toBe("light");
  });

  it("follows the platform only for the System preference", () => {
    const dark = { matches: true };
    const light = { matches: false };
    expect(resolveAppearance("system", dark)).toBe("dark");
    expect(resolveAppearance("system", light)).toBe("light");
    expect(resolveAppearance("light", dark)).toBe("light");
    expect(resolveAppearance("dark", light)).toBe("dark");
  });

  it("formats stored preferences for the accessible setting", () => {
    expect(appearanceLabel("system")).toBe("System");
    expect(appearanceLabel("dark")).toBe("Dark");
  });
});
