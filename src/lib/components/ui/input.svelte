<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type InputProps = HTMLInputAttributes;

  let { class: className, type = "text", ...restProps }: InputProps = $props();
</script>

<input
  class={cn(
    "h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs transition-colors outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50",
    className,
  )}
  {type}
  {...restProps}
/>
