// Ported from Thunderbolt (fork/rebrand src/components/ui/modal-styles.ts):
// gives form fields inside a detail panel the correct inset-surface tone so
// they read against the panel's bg-sidebar ground.
export const panelFieldSurfaceClass =
  "[&_[data-slot=input]]:bg-background [&_[data-slot=textarea]]:bg-background [&_[data-slot=select-trigger]]:bg-background [&_[data-slot=combobox-trigger]]:bg-background dark:[&_[data-slot=input]]:bg-input dark:[&_[data-slot=textarea]]:bg-input dark:[&_[data-slot=select-trigger]]:bg-input dark:[&_[data-slot=combobox-trigger]]:bg-input";
