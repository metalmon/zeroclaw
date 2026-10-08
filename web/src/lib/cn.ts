import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merge Tailwind class lists, letting later utilities win over conflicting
 * earlier ones (px-2 + px-4 -> px-4). Same helper Thunderbolt uses, so ported
 * component JSX (cva variants + className overrides) behaves identically here.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
