import type { AutoConvergeProfile } from "../../types/autoConverge";

interface ProfileSelectorProps {
  profiles: AutoConvergeProfile[];
  selectedProfileId: string;
  onSelectProfile: (profileId: string) => void;
  onSaveCustomProfile: (profile: AutoConvergeProfile) => void;
  onResetToPresets: () => void;
  disabled?: boolean;
}

export default function ProfileSelector({
  profiles,
  selectedProfileId,
  onSelectProfile,
  onSaveCustomProfile,
  onResetToPresets,
  disabled = false,
}: ProfileSelectorProps) {
  const activeProfile =
    profiles.find((p) => p.id === selectedProfileId) ?? profiles[0];

  const handleCloneAsCustom = () => {
    if (!activeProfile) return;
    const customId = `custom-${Date.now()}`;
    const cloned: AutoConvergeProfile = {
      ...activeProfile,
      id: customId,
      name: `${activeProfile.name} (Custom)`,
      is_builtin: false,
      plan_review_mandatory: true,
      implementation_review_mandatory: true,
      corrective_plan_review_mandatory: true,
    };
    onSaveCustomProfile(cloned);
  };

  return (
    <div className="auto-converge-profile-bar" data-testid="profile-selector">
      <label htmlFor="ac-profile-select">Profile:</label>
      <select
        id="ac-profile-select"
        data-testid="profile-select"
        value={selectedProfileId}
        disabled={disabled}
        onChange={(e) => onSelectProfile(e.target.value)}
      >
        {profiles.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
      <button
        type="button"
        className="btn btn-small"
        data-testid="clone-profile-btn"
        disabled={disabled || !activeProfile}
        onClick={handleCloneAsCustom}
      >
        Save As Custom
      </button>
      <button
        type="button"
        className="btn btn-small"
        data-testid="reset-presets-btn"
        disabled={disabled}
        onClick={onResetToPresets}
      >
        Reset to Presets
      </button>
    </div>
  );
}
