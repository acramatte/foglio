export type AppearancePreference = "system" | "light" | "dark";
export type ResolvedAppearance = "light" | "dark";

export function systemAppearance(
  media: Pick<MediaQueryList, "matches"> | null | undefined,
): ResolvedAppearance {
  return media?.matches ? "dark" : "light";
}

export function resolveAppearance(
  preference: AppearancePreference,
  media: Pick<MediaQueryList, "matches"> | null | undefined,
): ResolvedAppearance {
  return preference === "system" ? systemAppearance(media) : preference;
}

export function appearanceLabel(preference: AppearancePreference): string {
  return preference[0].toUpperCase() + preference.slice(1);
}
