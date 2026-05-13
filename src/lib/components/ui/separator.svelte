<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type SeparatorProps = HTMLAttributes<HTMLDivElement> & {
    orientation?: "horizontal" | "vertical";
  };

  let {
    class: className,
    orientation = "horizontal",
    ...restProps
  }: SeparatorProps = $props();
</script>

<div
  role="separator"
  aria-orientation={orientation}
  class={cn(
    "shrink-0 bg-border",
    orientation === "horizontal" ? "h-px w-full" : "h-full w-px",
    className,
  )}
  {...restProps}
></div>
