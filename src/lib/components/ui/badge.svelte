<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import { tv, type VariantProps } from "tailwind-variants";
  import { cn } from "$lib/utils";

  export const badgeVariants = tv({
    base: "inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-md border px-2 py-0.5 text-xs font-medium whitespace-nowrap transition-colors",
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground",
        secondary: "border-transparent bg-secondary text-secondary-foreground",
        outline: "text-foreground",
        muted: "border-transparent bg-muted text-muted-foreground",
        destructive: "border-transparent bg-destructive text-destructive-foreground",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  });

  type BadgeProps = HTMLAttributes<HTMLSpanElement> &
    VariantProps<typeof badgeVariants> & {
      children?: Snippet;
    };

  let { children, class: className, variant = "default", ...restProps }: BadgeProps = $props();
</script>

<span class={cn(badgeVariants({ variant }), className)} {...restProps}>
  {@render children?.()}
</span>
