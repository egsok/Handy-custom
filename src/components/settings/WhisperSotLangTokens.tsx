import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { SettingContainer } from "../ui/SettingContainer";
import { Dropdown } from "../ui/Dropdown";
import type { DropdownOption } from "../ui/Dropdown";

interface WhisperSotLangTokensProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const PRESETS: Record<string, string> = {
  off: "",
  ru_en: "ru, en",
  en_ru: "en, ru",
  ru_en_zh: "ru, en, zh",
};

function parseTokens(raw: string): string[] {
  return raw
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter((s) => s.length > 0);
}

function tokensToInput(tokens: string[] | null | undefined): string {
  if (!tokens || tokens.length === 0) return "";
  return tokens.join(", ");
}

export const WhisperSotLangTokens: React.FC<WhisperSotLangTokensProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const persisted = getSetting("whisper_sot_lang_tokens") ?? null;
    const [localValue, setLocalValue] = useState(tokensToInput(persisted));
    const [isDirty, setIsDirty] = useState(false);

    useEffect(() => {
      if (!isDirty) {
        setLocalValue(tokensToInput(persisted));
      }
    }, [persisted, isDirty]);

    const activePreset = useMemo(() => {
      const normalised = parseTokens(localValue).join(",");
      const match = Object.entries(PRESETS).find(
        ([, preset]) => parseTokens(preset).join(",") === normalised,
      );
      return match?.[0] ?? "custom";
    }, [localValue]);

    const presetOptions: DropdownOption[] = useMemo(
      () => [
        {
          value: "off",
          label: t("settings.advanced.whisperSotLangTokens.presets.off"),
        },
        {
          value: "ru_en",
          label: t("settings.advanced.whisperSotLangTokens.presets.ruEn"),
        },
        {
          value: "en_ru",
          label: t("settings.advanced.whisperSotLangTokens.presets.enRu"),
        },
        {
          value: "ru_en_zh",
          label: t("settings.advanced.whisperSotLangTokens.presets.ruEnZh"),
        },
        ...(activePreset === "custom"
          ? [
              {
                value: "custom",
                label: t(
                  "settings.advanced.whisperSotLangTokens.presets.custom",
                ),
              },
            ]
          : []),
      ],
      [t, activePreset],
    );

    const persist = useCallback(
      (raw: string) => {
        const tokens = parseTokens(raw);
        updateSetting(
          "whisper_sot_lang_tokens",
          tokens.length > 0 ? tokens : null,
        );
      },
      [updateSetting],
    );

    const handleChange = useCallback(
      (e: React.ChangeEvent<HTMLInputElement>) => {
        setLocalValue(e.target.value);
        setIsDirty(true);
      },
      [],
    );

    const handleBlur = useCallback(() => {
      if (!isDirty) return;
      persist(localValue);
      setIsDirty(false);
    }, [localValue, isDirty, persist]);

    const handlePreset = useCallback(
      (key: string) => {
        const preset = PRESETS[key] ?? "";
        setLocalValue(preset);
        persist(preset);
        setIsDirty(false);
      },
      [persist],
    );

    return (
      <SettingContainer
        title={t("settings.advanced.whisperSotLangTokens.title")}
        description={t("settings.advanced.whisperSotLangTokens.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <div className="flex flex-col gap-2 w-full">
          <div className="flex items-center gap-2">
            <label className="text-xs text-mid-gray">
              {t("settings.advanced.whisperSotLangTokens.presets.label")}
            </label>
            <Dropdown
              options={presetOptions}
              selectedValue={activePreset}
              onSelect={handlePreset}
              disabled={isUpdating("whisper_sot_lang_tokens")}
              className="min-w-[140px]"
            />
          </div>
          <input
            type="text"
            className="w-full px-2 py-1 text-sm rounded border border-mid-gray/30 bg-mid-gray/5 focus:outline-none focus:border-logo-primary"
            value={localValue}
            onChange={handleChange}
            onBlur={handleBlur}
            placeholder={t(
              "settings.advanced.whisperSotLangTokens.placeholder",
            )}
            disabled={isUpdating("whisper_sot_lang_tokens")}
            spellCheck={false}
            autoCorrect="off"
          />
          <span className="text-mid-gray/60 text-xs">
            {t("settings.advanced.whisperSotLangTokens.hint")}
          </span>
        </div>
      </SettingContainer>
    );
  });
