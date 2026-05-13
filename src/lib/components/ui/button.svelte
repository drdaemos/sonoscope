<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { tv, type VariantProps } from "tailwind-variants";
  import { cn } from "$lib/utils";

  export const buttonVariants = tv({
    base: "inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
        outline: "border bg-background hover:bg-accent hover:text-accent-foreground",
        secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 rounded-md px-3",
        icon: "size-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  });

  type ButtonProps = HTMLButtonAttributes &
    VariantProps<typeof buttonVariants> & {
      children?: Snippet;
    };

  let {
    children,
    class: className,
    variant = "default",
    size = "default",
    type = "button",
    ...restProps
  }: ButtonProps = $props();
</script>

<button class={cn(buttonVariants({ variant, size }), className)} {type} {...restProps}>
  {@render children?.()}
</button>
