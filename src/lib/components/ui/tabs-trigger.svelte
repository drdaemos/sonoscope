<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type TabsTriggerProps = HTMLButtonAttributes & {
    active?: boolean;
    children?: Snippet;
  };

  let {
    active = false,
    children,
    class: className,
    type = "button",
    ...restProps
  }: TabsTriggerProps = $props();
</script>

<button
  class={cn(
    "inline-flex h-7 min-w-20 items-center justify-center rounded-md px-3 text-sm font-medium whitespace-nowrap transition-colors outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] disabled:pointer-events-none disabled:opacity-50",
    active
      ? "bg-background text-foreground shadow-sm"
      : "text-muted-foreground hover:text-foreground",
    className,
  )}
  {type}
  aria-current={active ? "page" : undefined}
  {...restProps}
>
  {@render children?.()}
</button>
