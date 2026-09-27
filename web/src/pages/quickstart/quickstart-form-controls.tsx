import { Badge } from "../../components/ui/Badge";
import { t } from "../../lib/i18n";

const INPUT_CLASS =
  "w-full h-9 px-3 rounded-[var(--radius-md)] border border-border bg-input text-sm text-foreground placeholder:text-text-faint focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:border-primary/40";
const TEXTAREA_CLASS =
  "w-full px-3 py-2 rounded-[var(--radius-md)] border border-border bg-input text-sm text-foreground placeholder:text-text-faint focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:border-primary/40";
const MUTED = { color: "var(--color-muted-foreground)" } as const;

export function LabeledInput({
  label,
  value,
  onChange,
  type = "text",
  placeholder,
  multiline = false,
  help,
  required = false,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  type?: "text" | "password";
  placeholder?: string;
  multiline?: boolean;
  help?: string;
  required?: boolean;
}) {
  return (
    <label className="block">
      <div className="text-xs uppercase tracking-wider mb-1" style={MUTED}>
        {label}
        {required && (
          <Badge tone="warn" className="ml-2 uppercase tracking-wide">
            {t("fieldform.badge_required")}
          </Badge>
        )}
      </div>
      {help ? (
        <div className="text-xs mb-1 italic" style={MUTED}>
          {help}
        </div>
      ) : null}
      {multiline ? (
        <textarea
          className={`${TEXTAREA_CLASS} min-h-24`}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          required={required}
          aria-required={required}
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
        />
      ) : (
        <input
          className={INPUT_CLASS}
          type={type}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          required={required}
          aria-required={required}
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
        />
      )}
    </label>
  );
}
