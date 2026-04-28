import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface WhisperAntiHallucinationProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const WhisperAntiHallucination: React.FC<WhisperAntiHallucinationProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("whisper_anti_hallucination") || false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(v) => updateSetting("whisper_anti_hallucination", v)}
        isUpdating={isUpdating("whisper_anti_hallucination")}
        label={t("settings.advanced.whisperAntiHallucination.label")}
        description={t(
          "settings.advanced.whisperAntiHallucination.description",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
