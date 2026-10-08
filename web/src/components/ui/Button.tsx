import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import {
  type ComponentProps,
  type MouseEvent,
  useCallback,
} from "react";

import { cn } from "../../lib/cn.ts";

// Variants ported verbatim from Thunderbolt (fork/rebrand src/components/ui/button.tsx)
// so its component JSX drops in here. App-specific bits (haptics, lucide loader)
// are dropped; the class recipes are the design contract we keep in sync.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg text-[length:var(--font-size-body)] font-medium transition-all cursor-pointer disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
  {
    variants: {
      variant: {
        default:
          "border border-transparent bg-brand bg-origin-border text-brand-foreground shadow-xs [background-image:var(--gradient-brand)] hover:brightness-[1.06] active:brightness-95 disabled:bg-secondary disabled:text-muted-foreground disabled:shadow-none disabled:opacity-100 disabled:brightness-100 disabled:[background-image:none]",
        destructive:
          "bg-destructive text-white shadow-xs hover:bg-destructive/90 dark:bg-destructive/60",
        outline:
          "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-card/30 dark:hover:bg-card/50",
        secondary:
          "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-[var(--touch-height-default)] px-4 py-2 has-[>svg]:px-3",
        sm: "h-[var(--touch-height-sm)] gap-1.5 px-3 has-[>svg]:px-2",
        xs: "h-7 gap-1 px-2 text-[length:var(--font-size-xs)] has-[>svg]:px-1.5",
        lg: "h-[var(--touch-height-lg)] px-6 has-[>svg]:px-4",
        icon: "size-[var(--touch-height-default)]",
        "icon-sm": "size-[var(--touch-height-sm)]",
        "icon-lg": "size-[var(--touch-height-lg)]",
        "icon-xs": "size-7",
      },
      loading: {
        true: "",
        false: "",
      },
    },
    compoundVariants: [
      {
        variant: "default",
        loading: true,
        class:
          "disabled:bg-brand disabled:text-brand-foreground/80 disabled:brightness-75 disabled:saturate-50 disabled:[background-image:var(--gradient-brand)]",
      },
    ],
    defaultVariants: {
      variant: "default",
      size: "default",
      loading: false,
    },
  },
);

export const mutedIconButtonClass =
  "size-[var(--touch-height-lg)] md:size-[var(--touch-height-sm)] rounded-full md:rounded-xl text-muted-foreground hover:bg-muted dark:hover:bg-muted hover:text-foreground active:bg-muted data-[state=open]:bg-muted data-[state=open]:text-foreground [&_svg:not([class*='size-'])]:size-[var(--icon-size-default)]";

function Spinner() {
  return (
    <svg
      className="animate-spin"
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      aria-hidden="true"
    >
      <path d="M21 12a9 9 0 1 1-6.2-8.6" strokeLinecap="round" />
    </svg>
  );
}

export type ButtonProps = ComponentProps<"button"> &
  Omit<VariantProps<typeof buttonVariants>, "loading"> & {
    asChild?: boolean;
    isLoading?: boolean;
    loadingLabel?: string;
  };

const Button = ({
  className,
  variant,
  size,
  asChild = false,
  isLoading = false,
  loadingLabel,
  disabled,
  children,
  onClick,
  ...props
}: ButtonProps) => {
  const Comp = asChild ? Slot : "button";
  const isDisabled = disabled || isLoading;

  const handleClick = useCallback(
    (e: MouseEvent<HTMLButtonElement>) => {
      if (isDisabled) {
        e.preventDefault();
        e.stopPropagation();
        return;
      }
      onClick?.(e);
    },
    [isDisabled, onClick],
  );

  return (
    <Comp
      {...props}
      data-slot="button"
      className={cn(buttonVariants({ variant, size, loading: isLoading, className }))}
      onClick={handleClick}
      disabled={asChild ? undefined : isDisabled}
      aria-disabled={asChild && isDisabled ? true : undefined}
      aria-busy={isLoading || undefined}
      tabIndex={asChild && isDisabled ? -1 : props.tabIndex}
    >
      {asChild ? (
        children
      ) : (
        <>
          {isLoading && <Spinner />}
          {isLoading && loadingLabel ? loadingLabel : children}
        </>
      )}
    </Comp>
  );
};

export { Button, buttonVariants };
